//! Typed, authenticated execution boundary for C6.
//!
//! This crate deliberately does not execute host commands or access a container
//! socket. [`SimulationBackend`] is the only backend in the first release. It
//! exercises lifecycle, persistence, cancellation, log, and policy behavior
//! without turning protocol input into host process execution.

use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{self, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use sha2::Sha256;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::{Mutex, watch},
    time,
};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_CLOCK_SKEW_MS: i64 = 60_000;
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
pub const MAX_TIMEOUT_SECONDS: u32 = 3_600;
const MIN_AUTH_KEY_BYTES: usize = 32;
const MAX_REPLAY_ENTRIES: usize = 4_096;

/// Loads an existing runner authentication key after enforcing its filesystem
/// safety requirements. This is the helper the unprivileged control plane uses
/// to share the runner-created key.
pub fn load_auth_key_file(path: &Path) -> Result<Vec<u8>, KeyFileError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(KeyFileError::UnsafeType);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(KeyFileError::InsecurePermissions);
        }
    }
    let key = std::fs::read(path)?;
    validate_auth_key(&key).map_err(|_| KeyFileError::WeakKey)?;
    Ok(key)
}

/// Loads or atomically creates a 32-byte runner authentication key.
///
/// Creation uses a complete owner-only temporary inode plus an atomic hard link
/// so concurrent starters can never observe a partially written key and an
/// attacker-controlled existing path is never overwritten.
pub fn load_or_create_auth_key(path: &Path) -> Result<Vec<u8>, KeyFileError> {
    match load_auth_key_file(path) {
        Ok(key) => return Ok(key),
        Err(KeyFileError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = path.parent().ok_or(KeyFileError::UnsafeParent)?;
    let parent_metadata = std::fs::symlink_metadata(parent)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(KeyFileError::UnsafeParent);
    }

    let temporary = parent.join(format!(".c6-runner-key-{}.tmp", Uuid::new_v4()));
    let cleanup = TemporaryKey(temporary.clone());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    let mut generated = [0_u8; 32];
    getrandom::fill(&mut generated).map_err(|error| KeyFileError::Random(error.to_string()))?;
    file.write_all(&generated)?;
    file.sync_all()?;
    drop(file);

    match std::fs::hard_link(&temporary, path) {
        Ok(()) => {
            drop(cleanup);
            load_auth_key_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            drop(cleanup);
            load_auth_key_file(path)
        }
        Err(error) => Err(error.into()),
    }
}

struct TemporaryKey(PathBuf);

impl Drop for TemporaryKey {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KeyFileError {
    #[error("runner key I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("runner key path must be a regular file, never a symlink")]
    UnsafeType,
    #[error("runner key file permissions must be exactly 0600")]
    InsecurePermissions,
    #[error("runner key parent must be an existing real directory")]
    UnsafeParent,
    #[error("runner key must contain at least {MIN_AUTH_KEY_BYTES} bytes")]
    WeakKey,
    #[error("operating-system randomness failed: {0}")]
    Random(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedEnvelope {
    pub version: u8,
    pub request_id: Uuid,
    pub issued_at_ms: i64,
    /// Base64url-encoded 16-byte random value, unique within the skew window.
    pub nonce: String,
    /// Base64url-encoded JSON bytes for a [`RunnerCommand`].
    pub payload: String,
    /// Base64url-encoded HMAC-SHA256 over the other envelope fields.
    pub mac: String,
}

impl SignedEnvelope {
    pub fn sign<T: Serialize>(
        request_id: Uuid,
        issued_at_ms: i64,
        nonce: [u8; 16],
        payload: &T,
        key: &[u8],
    ) -> Result<Self, ProtocolError> {
        validate_auth_key(key)?;
        let payload = serde_json::to_vec(payload)
            .map_err(|error| ProtocolError::Malformed(error.to_string()))?;
        let nonce = URL_SAFE_NO_PAD.encode(nonce);
        let payload = URL_SAFE_NO_PAD.encode(payload);
        let mut envelope = Self {
            version: PROTOCOL_VERSION,
            request_id,
            issued_at_ms,
            nonce,
            payload,
            mac: String::new(),
        };
        envelope.mac = URL_SAFE_NO_PAD.encode(compute_mac(key, &envelope)?);
        Ok(envelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerCommand {
    Ping,
    Execute { execution: Box<ExecutionRequest> },
    Cancel { run_id: Uuid },
    Inspect { run_id: Uuid },
}

impl<'de> Deserialize<'de> for RunnerCommand {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Empty {}
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Execute {
            execution: Box<ExecutionRequest>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RunId {
            run_id: Uuid,
        }
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum StrictCommand {
            Ping(Empty),
            Execute(Execute),
            Cancel(RunId),
            Inspect(RunId),
        }
        Ok(match StrictCommand::deserialize(deserializer)? {
            StrictCommand::Ping(_) => Self::Ping,
            StrictCommand::Execute(value) => Self::Execute {
                execution: value.execution,
            },
            StrictCommand::Cancel(value) => Self::Cancel {
                run_id: value.run_id,
            },
            StrictCommand::Inspect(value) => Self::Inspect {
                run_id: value.run_id,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRequest {
    pub run_id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    /// Full, lowercase SHA-1 or SHA-256 Git object ID.
    pub revision_sha: String,
    /// Pinned sha256 digest of the manifest used to resolve this request.
    pub manifest_digest: String,
    pub resources: ResourcePolicy,
    pub network: NetworkPolicy,
    pub repository_write: RepositoryWritePolicy,
    pub simulation: SimulationPlan,
}

impl ExecutionRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.workspace_id.is_nil() || self.project_id.is_nil() || self.run_id.is_nil() {
            return Err(ValidationError::NilIdentifier);
        }
        validate_revision(&self.revision_sha)?;
        validate_digest(&self.manifest_digest)?;
        self.resources.validate()?;
        self.network.validate()?;
        self.simulation.validate(&self.resources)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePolicy {
    pub cpu_millis: u32,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub process_limit: u32,
    pub timeout_seconds: u32,
    pub log_bytes: u64,
}

impl ResourcePolicy {
    fn validate(&self) -> Result<(), ValidationError> {
        if !(10..=4_000).contains(&self.cpu_millis) {
            return Err(ValidationError::ResourceOutOfRange("cpu_millis"));
        }
        if !(16 * 1024 * 1024..=4 * 1024 * 1024 * 1024).contains(&self.memory_bytes) {
            return Err(ValidationError::ResourceOutOfRange("memory_bytes"));
        }
        if !(1024 * 1024..=20 * 1024 * 1024 * 1024).contains(&self.disk_bytes) {
            return Err(ValidationError::ResourceOutOfRange("disk_bytes"));
        }
        if !(1..=512).contains(&self.process_limit) {
            return Err(ValidationError::ResourceOutOfRange("process_limit"));
        }
        if !(1..=MAX_TIMEOUT_SECONDS).contains(&self.timeout_seconds) {
            return Err(ValidationError::ResourceOutOfRange("timeout_seconds"));
        }
        if !(1_024..=MAX_LOG_BYTES).contains(&self.log_bytes) {
            return Err(ValidationError::ResourceOutOfRange("log_bytes"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum NetworkPolicy {
    DenyAll,
    AllowList {
        destinations: Vec<NetworkDestination>,
    },
}

impl<'de> Deserialize<'de> for NetworkPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Empty {}
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Destinations {
            destinations: Vec<NetworkDestination>,
        }
        #[derive(Deserialize)]
        #[serde(tag = "mode", rename_all = "snake_case")]
        enum StrictPolicy {
            DenyAll(Empty),
            AllowList(Destinations),
        }
        Ok(match StrictPolicy::deserialize(deserializer)? {
            StrictPolicy::DenyAll(_) => Self::DenyAll,
            StrictPolicy::AllowList(value) => Self::AllowList {
                destinations: value.destinations,
            },
        })
    }
}

impl NetworkPolicy {
    fn validate(&self) -> Result<(), ValidationError> {
        let Self::AllowList { destinations } = self else {
            return Ok(());
        };
        if destinations.is_empty() || destinations.len() > 32 {
            return Err(ValidationError::InvalidNetworkPolicy);
        }
        for destination in destinations {
            destination.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDestination {
    pub host: String,
    pub port: u16,
}

impl NetworkDestination {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.port == 0
            || self.host.is_empty()
            || self.host.len() > 253
            || self.host != self.host.to_ascii_lowercase()
            || self.host.starts_with('.')
            || self.host.ends_with('.')
            || self.host.contains("..")
            || self.host.contains('*')
            || !self.host.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
            })
        {
            return Err(ValidationError::InvalidDestination);
        }
        let blocked = [
            "localhost",
            "localhost.localdomain",
            "metadata.google.internal",
            "169.254.169.254",
            "127.0.0.1",
            "::1",
        ];
        if blocked.contains(&self.host.as_str()) {
            return Err(ValidationError::BlockedDestination);
        }
        if let Ok(address) = self.host.parse::<IpAddr>() {
            let protected = match address {
                IpAddr::V4(address) => {
                    address.is_private()
                        || address.is_loopback()
                        || address.is_link_local()
                        || address.is_unspecified()
                        || address.is_multicast()
                }
                IpAddr::V6(address) => {
                    address.is_unique_local()
                        || address.is_loopback()
                        || address.is_unicast_link_local()
                        || address.is_unspecified()
                        || address.is_multicast()
                }
            };
            if protected {
                return Err(ValidationError::BlockedDestination);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryWritePolicy {
    None,
    Proposal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationPlan {
    pub delay_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl SimulationPlan {
    fn validate(&self, resources: &ResourcePolicy) -> Result<(), ValidationError> {
        let input_bytes = self.stdout.len().saturating_add(self.stderr.len()) as u64;
        if input_bytes > resources.log_bytes.saturating_mul(4).min(MAX_LOG_BYTES * 2) {
            return Err(ValidationError::SimulationInputTooLarge);
        }
        if !(-255..=255).contains(&self.exit_code) {
            return Err(ValidationError::InvalidExitCode);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedLog {
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub request: ExecutionRequest,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub stdout: CapturedLog,
    pub stderr: CapturedLog,
    pub started_at_ms: i64,
    pub finished_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerResponse {
    pub version: u8,
    pub request_id: Uuid,
    #[serde(flatten)]
    pub body: ResponseBody,
}

/// Typed control-plane client for the local runner boundary.
///
/// The authentication key is intentionally private and this type does not
/// implement `Debug`, preventing accidental credential logging through normal
/// diagnostic formatting.
pub struct RunnerClient {
    socket_path: PathBuf,
    auth_key: Vec<u8>,
}

impl RunnerClient {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        auth_key: impl Into<Vec<u8>>,
    ) -> Result<Self, ClientError> {
        let auth_key = auth_key.into();
        validate_auth_key(&auth_key)?;
        Ok(Self {
            socket_path: socket_path.into(),
            auth_key,
        })
    }

    pub async fn ping(&self) -> Result<ResponseBody, ClientError> {
        self.send(RunnerCommand::Ping).await
    }

    pub async fn execute(&self, execution: ExecutionRequest) -> Result<ResponseBody, ClientError> {
        execution.validate()?;
        self.send(RunnerCommand::Execute {
            execution: Box::new(execution),
        })
        .await
    }

    pub async fn inspect(&self, run_id: Uuid) -> Result<ResponseBody, ClientError> {
        self.send(RunnerCommand::Inspect { run_id }).await
    }

    pub async fn cancel(&self, run_id: Uuid) -> Result<ResponseBody, ClientError> {
        self.send(RunnerCommand::Cancel { run_id }).await
    }

    async fn send(&self, command: RunnerCommand) -> Result<ResponseBody, ClientError> {
        let request_id = Uuid::new_v4();
        let nonce = *Uuid::new_v4().as_bytes();
        let envelope =
            SignedEnvelope::sign(request_id, unix_millis(), nonce, &command, &self.auth_key)?;
        let mut frame = serde_json::to_vec(&envelope)?;
        if frame.len() > MAX_FRAME_BYTES {
            return Err(ClientError::FrameTooLarge);
        }
        frame.push(b'\n');

        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(&frame).await?;
        let response_bytes = read_frame(&mut stream).await?;
        let response: RunnerResponse = serde_json::from_slice(&response_bytes)?;
        if response.version != PROTOCOL_VERSION {
            return Err(ClientError::ResponseVersion);
        }
        if response.request_id != request_id {
            return Err(ClientError::ResponseRequestId);
        }
        Ok(response.body)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error("runner request exceeds {MAX_FRAME_BYTES} bytes")]
    FrameTooLarge,
    #[error("runner returned a different protocol version")]
    ResponseVersion,
    #[error("runner response did not match the request ID")]
    ResponseRequestId,
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("runner connection failed: {0}")]
    Io(#[from] io::Error),
    #[error("runner JSON encoding failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseBody {
    Pong,
    Finished { record: RunRecord },
    Status { record: Option<RunRecord> },
    CancelAcknowledged { already_terminal: bool },
    Rejected { code: ErrorCode, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    AuthenticationFailed,
    ReplayDetected,
    StaleRequest,
    UnsupportedVersion,
    MalformedRequest,
    InvalidExecution,
    IdempotencyConflict,
    NotFound,
    PersistenceFailed,
    Internal,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("authentication key must contain at least {MIN_AUTH_KEY_BYTES} bytes")]
    WeakKey,
    #[error("request authentication failed")]
    AuthenticationFailed,
    #[error("request nonce has already been used")]
    ReplayDetected,
    #[error("request timestamp is outside the allowed window")]
    StaleRequest,
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("malformed request: {0}")]
    Malformed(String),
}

impl ProtocolError {
    fn code(&self) -> ErrorCode {
        match self {
            Self::AuthenticationFailed | Self::WeakKey => ErrorCode::AuthenticationFailed,
            Self::ReplayDetected => ErrorCode::ReplayDetected,
            Self::StaleRequest => ErrorCode::StaleRequest,
            Self::UnsupportedVersion => ErrorCode::UnsupportedVersion,
            Self::Malformed(_) => ErrorCode::MalformedRequest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("run, workspace, and project identifiers must be non-nil")]
    NilIdentifier,
    #[error("revision must be a full lowercase SHA-1 or SHA-256 object ID")]
    InvalidRevision,
    #[error("digest must be pinned as sha256:<64 lowercase hex characters>")]
    InvalidDigest,
    #[error("resource {0} is outside runner limits")]
    ResourceOutOfRange(&'static str),
    #[error("network allow list must contain between 1 and 32 destinations")]
    InvalidNetworkPolicy,
    #[error("network destination must be a lowercase hostname and nonzero port")]
    InvalidDestination,
    #[error("network destination targets a protected local or metadata address")]
    BlockedDestination,
    #[error("simulation log input exceeds the accepted request bound")]
    SimulationInputTooLarge,
    #[error("simulation exit code is outside the supported range")]
    InvalidExitCode,
}

pub struct Authenticator {
    key: Vec<u8>,
    seen_nonces: Mutex<HashMap<String, i64>>,
}

impl Authenticator {
    pub fn new(key: impl Into<Vec<u8>>) -> Result<Self, ProtocolError> {
        let key = key.into();
        validate_auth_key(&key)?;
        Ok(Self {
            key,
            seen_nonces: Mutex::new(HashMap::new()),
        })
    }

    async fn verify<T: DeserializeOwned>(
        &self,
        envelope: &SignedEnvelope,
        now_ms: i64,
    ) -> Result<T, ProtocolError> {
        if envelope.version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if now_ms.abs_diff(envelope.issued_at_ms) > MAX_CLOCK_SKEW_MS as u64 {
            return Err(ProtocolError::StaleRequest);
        }
        let nonce = URL_SAFE_NO_PAD
            .decode(&envelope.nonce)
            .map_err(|_| ProtocolError::AuthenticationFailed)?;
        if nonce.len() != 16 {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let supplied_mac = URL_SAFE_NO_PAD
            .decode(&envelope.mac)
            .map_err(|_| ProtocolError::AuthenticationFailed)?;
        let expected_mac = compute_mac(&self.key, envelope)?;
        let mut verifier =
            HmacSha256::new_from_slice(&self.key).map_err(|_| ProtocolError::WeakKey)?;
        verifier.update(signing_input(envelope).as_bytes());
        verifier
            .verify_slice(&supplied_mac)
            .map_err(|_| ProtocolError::AuthenticationFailed)?;
        // Keep this assertion tied to the single signing implementation.
        debug_assert_eq!(expected_mac.as_slice(), supplied_mac.as_slice());

        let mut seen = self.seen_nonces.lock().await;
        seen.retain(|_, timestamp| now_ms.abs_diff(*timestamp) <= MAX_CLOCK_SKEW_MS as u64);
        if seen.contains_key(&envelope.nonce) {
            return Err(ProtocolError::ReplayDetected);
        }
        if seen.len() >= MAX_REPLAY_ENTRIES {
            return Err(ProtocolError::ReplayDetected);
        }
        seen.insert(envelope.nonce.clone(), envelope.issued_at_ms);
        drop(seen);

        let payload = URL_SAFE_NO_PAD
            .decode(&envelope.payload)
            .map_err(|error| ProtocolError::Malformed(error.to_string()))?;
        serde_json::from_slice(&payload)
            .map_err(|error| ProtocolError::Malformed(error.to_string()))
    }
}

fn signing_input(envelope: &SignedEnvelope) -> String {
    format!(
        "c6-runner-v1\n{}\n{}\n{}\n{}",
        envelope.request_id, envelope.issued_at_ms, envelope.nonce, envelope.payload
    )
}

fn compute_mac(key: &[u8], envelope: &SignedEnvelope) -> Result<Vec<u8>, ProtocolError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| ProtocolError::WeakKey)?;
    mac.update(signing_input(envelope).as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

fn validate_auth_key(key: &[u8]) -> Result<(), ProtocolError> {
    if key.len() < MIN_AUTH_KEY_BYTES {
        Err(ProtocolError::WeakKey)
    } else {
        Ok(())
    }
}

fn validate_revision(value: &str) -> Result<(), ValidationError> {
    if matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(ValidationError::InvalidRevision)
    }
}

fn validate_digest(value: &str) -> Result<(), ValidationError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(ValidationError::InvalidDigest);
    };
    if hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(ValidationError::InvalidDigest)
    }
}

#[derive(Debug)]
pub struct BackendOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, thiserror::Error)]
#[error("simulation backend failed: {message}")]
pub struct BackendError {
    pub message: String,
}

#[async_trait]
pub trait ExecutionBackend: Send + Sync + 'static {
    async fn execute(&self, request: &ExecutionRequest) -> Result<BackendOutput, BackendError>;
}

#[derive(Debug, Default)]
pub struct SimulationBackend;

#[async_trait]
impl ExecutionBackend for SimulationBackend {
    async fn execute(&self, request: &ExecutionRequest) -> Result<BackendOutput, BackendError> {
        time::sleep(Duration::from_millis(request.simulation.delay_ms)).await;
        Ok(BackendOutput {
            exit_code: request.simulation.exit_code,
            stdout: request.simulation.stdout.clone(),
            stderr: request.simulation.stderr.clone(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("result persistence failed: {0}")]
pub struct StoreError(String);

#[async_trait]
pub trait ResultStore: Send + Sync + 'static {
    async fn load(&self, run_id: Uuid) -> Result<Option<RunRecord>, StoreError>;
    async fn save(&self, record: &RunRecord) -> Result<(), StoreError>;
}

#[derive(Debug, Clone)]
pub struct FileResultStore {
    root: PathBuf,
}

impl FileResultStore {
    pub async fn new(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        tokio::fs::create_dir_all(&root)
            .await
            .map_err(|error| StoreError(error.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|error| StoreError(error.to_string()))?;
        }
        Ok(Self { root })
    }

    fn path(&self, run_id: Uuid) -> PathBuf {
        self.root.join(format!("{run_id}.json"))
    }
}

#[async_trait]
impl ResultStore for FileResultStore {
    async fn load(&self, run_id: Uuid) -> Result<Option<RunRecord>, StoreError> {
        match tokio::fs::read(self.path(run_id)).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| StoreError(error.to_string())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }

    async fn save(&self, record: &RunRecord) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec(record).map_err(|error| StoreError(error.to_string()))?;
        let path = self.path(record.request.run_id);
        let temporary = self.root.join(format!(".{}.tmp", record.request.run_id));
        tokio::fs::write(&temporary, bytes)
            .await
            .map_err(|error| StoreError(error.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
                .await
                .map_err(|error| StoreError(error.to_string()))?;
        }
        tokio::fs::rename(&temporary, &path)
            .await
            .map_err(|error| StoreError(error.to_string()))
    }
}

struct ActiveRun {
    request: ExecutionRequest,
    cancel: watch::Sender<bool>,
}

pub struct RunnerService<B, S> {
    authenticator: Authenticator,
    backend: Arc<B>,
    store: Arc<S>,
    active: Mutex<HashMap<Uuid, ActiveRun>>,
}

impl<B: ExecutionBackend, S: ResultStore> RunnerService<B, S> {
    pub fn new(authenticator: Authenticator, backend: Arc<B>, store: Arc<S>) -> Self {
        Self {
            authenticator,
            backend,
            store,
            active: Mutex::new(HashMap::new()),
        }
    }

    pub async fn handle(&self, envelope: SignedEnvelope) -> RunnerResponse {
        let request_id = envelope.request_id;
        let command = match self
            .authenticator
            .verify::<RunnerCommand>(&envelope, unix_millis())
            .await
        {
            Ok(command) => command,
            Err(error) => return rejected(request_id, error.code(), error.to_string()),
        };
        let body = match command {
            RunnerCommand::Ping => ResponseBody::Pong,
            RunnerCommand::Inspect { run_id } => self.inspect(run_id).await,
            RunnerCommand::Cancel { run_id } => self.cancel(run_id).await,
            RunnerCommand::Execute { execution } => self.execute(*execution).await,
        };
        RunnerResponse {
            version: PROTOCOL_VERSION,
            request_id,
            body,
        }
    }

    async fn inspect(&self, run_id: Uuid) -> ResponseBody {
        match self.store.load(run_id).await {
            Ok(record) => ResponseBody::Status { record },
            Err(error) => ResponseBody::Rejected {
                code: ErrorCode::PersistenceFailed,
                message: error.to_string(),
            },
        }
    }

    async fn cancel(&self, run_id: Uuid) -> ResponseBody {
        let active = self.active.lock().await;
        if let Some(run) = active.get(&run_id) {
            let _ = run.cancel.send(true);
            return ResponseBody::CancelAcknowledged {
                already_terminal: false,
            };
        }
        drop(active);
        match self.store.load(run_id).await {
            Ok(Some(_)) => ResponseBody::CancelAcknowledged {
                already_terminal: true,
            },
            Ok(None) => ResponseBody::Rejected {
                code: ErrorCode::NotFound,
                message: "run was not found".into(),
            },
            Err(error) => ResponseBody::Rejected {
                code: ErrorCode::PersistenceFailed,
                message: error.to_string(),
            },
        }
    }

    async fn execute(&self, execution: ExecutionRequest) -> ResponseBody {
        if let Err(error) = execution.validate() {
            return ResponseBody::Rejected {
                code: ErrorCode::InvalidExecution,
                message: error.to_string(),
            };
        }
        match self.store.load(execution.run_id).await {
            Ok(Some(record)) if record.request == execution => {
                return ResponseBody::Finished { record };
            }
            Ok(Some(_)) => {
                return ResponseBody::Rejected {
                    code: ErrorCode::IdempotencyConflict,
                    message: "run ID is already bound to a different execution".into(),
                };
            }
            Err(error) => {
                return ResponseBody::Rejected {
                    code: ErrorCode::PersistenceFailed,
                    message: error.to_string(),
                };
            }
            Ok(None) => {}
        }

        let (cancel, mut cancellation) = watch::channel(false);
        {
            let mut active = self.active.lock().await;
            if let Some(existing) = active.get(&execution.run_id) {
                return ResponseBody::Rejected {
                    code: ErrorCode::IdempotencyConflict,
                    message: if existing.request == execution {
                        "run is already active".into()
                    } else {
                        "run ID is active with a different execution".into()
                    },
                };
            }
            active.insert(
                execution.run_id,
                ActiveRun {
                    request: execution.clone(),
                    cancel,
                },
            );
        }

        let started_at_ms = unix_millis();
        let timeout = Duration::from_secs(u64::from(execution.resources.timeout_seconds));
        let outcome = tokio::select! {
            biased;
            changed = cancellation.changed() => {
                let _ = changed;
                RunOutcome::Cancelled
            }
            result = time::timeout(timeout, self.backend.execute(&execution)) => {
                match result {
                    Err(_) => RunOutcome::TimedOut,
                    Ok(Err(error)) => RunOutcome::BackendFailed(error.to_string()),
                    Ok(Ok(output)) => RunOutcome::Exited(output),
                }
            }
        };
        self.active.lock().await.remove(&execution.run_id);

        let record = build_record(execution, started_at_ms, outcome);
        if let Err(error) = self.store.save(&record).await {
            return ResponseBody::Rejected {
                code: ErrorCode::PersistenceFailed,
                message: error.to_string(),
            };
        }
        ResponseBody::Finished { record }
    }
}

enum RunOutcome {
    Exited(BackendOutput),
    BackendFailed(String),
    Cancelled,
    TimedOut,
}

fn build_record(request: ExecutionRequest, started_at_ms: i64, outcome: RunOutcome) -> RunRecord {
    let limit = request.resources.log_bytes as usize;
    let (status, exit_code, stdout, stderr) = match outcome {
        RunOutcome::Exited(output) => (
            if output.exit_code == 0 {
                RunStatus::Succeeded
            } else {
                RunStatus::Failed
            },
            Some(output.exit_code),
            capture_log(output.stdout, limit),
            capture_log(output.stderr, limit),
        ),
        RunOutcome::BackendFailed(message) => (
            RunStatus::Interrupted,
            None,
            capture_log(String::new(), limit),
            capture_log(message, limit),
        ),
        RunOutcome::Cancelled => (
            RunStatus::Cancelled,
            None,
            capture_log(String::new(), limit),
            capture_log(String::new(), limit),
        ),
        RunOutcome::TimedOut => (
            RunStatus::TimedOut,
            None,
            capture_log(String::new(), limit),
            capture_log(String::new(), limit),
        ),
    };
    RunRecord {
        request,
        status,
        exit_code,
        stdout,
        stderr,
        started_at_ms,
        finished_at_ms: unix_millis(),
    }
}

fn capture_log(mut content: String, limit: usize) -> CapturedLog {
    if content.len() <= limit {
        return CapturedLog {
            content,
            truncated: false,
        };
    }
    let mut boundary = limit;
    while !content.is_char_boundary(boundary) {
        boundary -= 1;
    }
    content.truncate(boundary);
    CapturedLog {
        content,
        truncated: true,
    }
}

fn rejected(request_id: Uuid, code: ErrorCode, message: String) -> RunnerResponse {
    RunnerResponse {
        version: PROTOCOL_VERSION,
        request_id,
        body: ResponseBody::Rejected { code, message },
    }
}

pub fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut frame = Vec::with_capacity(4_096);
    let mut chunk = [0_u8; 4_096];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Err(FrameError::Truncated);
        }
        let bytes = &chunk[..read];
        if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
            frame.extend_from_slice(&bytes[..newline]);
            if newline + 1 != bytes.len() {
                return Err(FrameError::MultipleFrames);
            }
            break;
        }
        frame.extend_from_slice(bytes);
        if frame.len() > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
    }
    if frame.is_empty() {
        return Err(FrameError::Malformed("empty frame".into()));
    }
    if frame.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    Ok(frame)
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    TooLarge,
    #[error("connection ended before newline terminator")]
    Truncated,
    #[error("only one frame is allowed per connection")]
    MultipleFrames,
    #[error("malformed frame: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
}

pub async fn serve<B: ExecutionBackend, S: ResultStore>(
    config: DaemonConfig,
    service: Arc<RunnerService<B, S>>,
) -> Result<(), DaemonError> {
    prepare_socket_path(&config.socket_path)?;
    let listener = UnixListener::bind(&config.socket_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config.socket_path, std::fs::Permissions::from_mode(0o600))?;
    }
    let cleanup = SocketCleanup(config.socket_path.clone());
    loop {
        let (stream, _) = listener.accept().await?;
        let service = Arc::clone(&service);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, service).await {
                tracing::warn!(%error, "runner connection rejected");
            }
        });
        let _ = &cleanup;
    }
}

async fn handle_connection<B: ExecutionBackend, S: ResultStore>(
    mut stream: UnixStream,
    service: Arc<RunnerService<B, S>>,
) -> Result<(), DaemonError> {
    let bytes = read_frame(&mut stream).await?;
    let envelope: SignedEnvelope =
        serde_json::from_slice(&bytes).map_err(|error| FrameError::Malformed(error.to_string()))?;
    let response = service.handle(envelope).await;
    let mut bytes = serde_json::to_vec(&response)?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await?;
    stream.shutdown().await?;
    Ok(())
}

fn prepare_socket_path(path: &Path) -> Result<(), DaemonError> {
    let parent = path.parent().ok_or(DaemonError::UnsafeSocketPath)?;
    if !parent.is_dir() {
        return Err(DaemonError::UnsafeSocketPath);
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::FileTypeExt;
                if !metadata.file_type().is_socket() {
                    return Err(DaemonError::UnsafeSocketPath);
                }
            }
            std::fs::remove_file(path)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

struct SocketCleanup(PathBuf);

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("socket path must have an existing parent and may only replace a Unix socket")]
    UnsafeSocketPath,
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("response encoding failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    const KEY: &[u8] = b"test-only-c6-runner-key-32-bytes-minimum";

    #[derive(Default)]
    struct MemoryStore(Mutex<HashMap<Uuid, RunRecord>>);

    #[async_trait]
    impl ResultStore for MemoryStore {
        async fn load(&self, run_id: Uuid) -> Result<Option<RunRecord>, StoreError> {
            Ok(self.0.lock().await.get(&run_id).cloned())
        }

        async fn save(&self, record: &RunRecord) -> Result<(), StoreError> {
            self.0
                .lock()
                .await
                .insert(record.request.run_id, record.clone());
            Ok(())
        }
    }

    fn request() -> ExecutionRequest {
        ExecutionRequest {
            run_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            revision_sha: "a".repeat(40),
            manifest_digest: format!("sha256:{}", "b".repeat(64)),
            resources: ResourcePolicy {
                cpu_millis: 500,
                memory_bytes: 64 * 1024 * 1024,
                disk_bytes: 1024 * 1024,
                process_limit: 16,
                timeout_seconds: 1,
                log_bytes: 1024,
            },
            network: NetworkPolicy::DenyAll,
            repository_write: RepositoryWritePolicy::None,
            simulation: SimulationPlan {
                delay_ms: 0,
                stdout: "ok".into(),
                stderr: String::new(),
                exit_code: 0,
            },
        }
    }

    fn service() -> Arc<RunnerService<SimulationBackend, MemoryStore>> {
        Arc::new(RunnerService::new(
            Authenticator::new(KEY).unwrap(),
            Arc::new(SimulationBackend),
            Arc::new(MemoryStore::default()),
        ))
    }

    async fn wait_for_socket(path: &Path) {
        for _ in 0..100 {
            if path.exists() {
                return;
            }
            time::sleep(Duration::from_millis(5)).await;
        }
        panic!("runner socket was not created");
    }

    fn signed(command: &RunnerCommand, nonce: u8) -> SignedEnvelope {
        SignedEnvelope::sign(Uuid::new_v4(), unix_millis(), [nonce; 16], command, KEY).unwrap()
    }

    #[test]
    fn validates_revision_and_digest_pinning() {
        let mut execution = request();
        assert_eq!(execution.validate(), Ok(()));
        execution.revision_sha = "abc123".into();
        assert_eq!(execution.validate(), Err(ValidationError::InvalidRevision));
        execution.revision_sha = "A".repeat(40);
        assert_eq!(execution.validate(), Err(ValidationError::InvalidRevision));
        execution.revision_sha = "a".repeat(40);
        execution.manifest_digest = "latest".into();
        assert_eq!(execution.validate(), Err(ValidationError::InvalidDigest));
    }

    #[test]
    fn validates_resource_bounds() {
        let mut execution = request();
        execution.resources.timeout_seconds = 0;
        assert!(matches!(
            execution.validate(),
            Err(ValidationError::ResourceOutOfRange("timeout_seconds"))
        ));
        execution = request();
        execution.resources.memory_bytes = u64::MAX;
        assert!(matches!(
            execution.validate(),
            Err(ValidationError::ResourceOutOfRange("memory_bytes"))
        ));
    }

    #[test]
    fn blocks_local_metadata_and_wildcard_destinations() {
        for host in [
            "localhost",
            "169.254.169.254",
            "10.20.30.40",
            "0.0.0.0",
            "metadata.google.internal",
            "*.example.com",
        ] {
            let mut execution = request();
            execution.network = NetworkPolicy::AllowList {
                destinations: vec![NetworkDestination {
                    host: host.into(),
                    port: 443,
                }],
            };
            assert!(execution.validate().is_err(), "host {host} should fail");
        }
    }

    #[tokio::test]
    async fn authenticates_and_detects_tampering_replay_and_staleness() {
        let service = service();
        let envelope = signed(&RunnerCommand::Ping, 1);
        assert!(matches!(
            service.handle(envelope.clone()).await.body,
            ResponseBody::Pong
        ));
        assert!(matches!(
            service.handle(envelope).await.body,
            ResponseBody::Rejected {
                code: ErrorCode::ReplayDetected,
                ..
            }
        ));

        let mut tampered = signed(&RunnerCommand::Ping, 2);
        tampered.payload.push('A');
        assert!(matches!(
            service.handle(tampered).await.body,
            ResponseBody::Rejected {
                code: ErrorCode::AuthenticationFailed,
                ..
            }
        ));

        let stale = SignedEnvelope::sign(
            Uuid::new_v4(),
            unix_millis() - MAX_CLOCK_SKEW_MS - 1,
            [3; 16],
            &RunnerCommand::Ping,
            KEY,
        )
        .unwrap();
        assert!(matches!(
            service.handle(stale).await.body,
            ResponseBody::Rejected {
                code: ErrorCode::StaleRequest,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn rejects_authenticated_unknown_fields() {
        let service = service();
        let command = serde_json::json!({"type": "ping", "unexpected": true});
        let envelope =
            SignedEnvelope::sign(Uuid::new_v4(), unix_millis(), [17; 16], &command, KEY).unwrap();
        assert!(matches!(
            service.handle(envelope).await.body,
            ResponseBody::Rejected {
                code: ErrorCode::MalformedRequest,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn executes_persists_and_replays_idempotently() {
        let service = service();
        let execution = request();
        let command = RunnerCommand::Execute {
            execution: Box::new(execution.clone()),
        };
        let first = service.handle(signed(&command, 4)).await;
        let ResponseBody::Finished { record } = first.body else {
            panic!("expected finished response")
        };
        assert_eq!(record.status, RunStatus::Succeeded);
        assert_eq!(record.exit_code, Some(0));
        assert_eq!(record.stdout.content, "ok");

        let second = service.handle(signed(&command, 5)).await;
        assert!(matches!(second.body, ResponseBody::Finished { .. }));
        let inspected = service
            .handle(signed(
                &RunnerCommand::Inspect {
                    run_id: execution.run_id,
                },
                6,
            ))
            .await;
        assert!(matches!(
            inspected.body,
            ResponseBody::Status { record: Some(_) }
        ));
    }

    #[tokio::test]
    async fn rejects_conflicting_run_identity() {
        let service = service();
        let execution = request();
        service
            .handle(signed(
                &RunnerCommand::Execute {
                    execution: Box::new(execution.clone()),
                },
                7,
            ))
            .await;
        let mut conflict = execution;
        conflict.project_id = Uuid::new_v4();
        let response = service
            .handle(signed(
                &RunnerCommand::Execute {
                    execution: Box::new(conflict),
                },
                8,
            ))
            .await;
        assert!(matches!(
            response.body,
            ResponseBody::Rejected {
                code: ErrorCode::IdempotencyConflict,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn bounds_unicode_logs_without_splitting_characters() {
        let service = service();
        let mut execution = request();
        execution.resources.log_bytes = 1024;
        execution.simulation.stdout = "🦀".repeat(300);
        let response = service
            .handle(signed(
                &RunnerCommand::Execute {
                    execution: Box::new(execution),
                },
                9,
            ))
            .await;
        let ResponseBody::Finished { record } = response.body else {
            panic!("expected finished response")
        };
        assert_eq!(record.stdout.content.len(), 1024);
        assert!(record.stdout.truncated);
        assert!(record.stdout.content.ends_with('🦀'));
    }

    #[tokio::test]
    async fn times_out_and_persists_terminal_result() {
        let service = service();
        let mut execution = request();
        execution.simulation.delay_ms = 1_100;
        let run_id = execution.run_id;
        let response = service
            .handle(signed(
                &RunnerCommand::Execute {
                    execution: Box::new(execution),
                },
                10,
            ))
            .await;
        assert!(matches!(
            response.body,
            ResponseBody::Finished {
                record: RunRecord {
                    status: RunStatus::TimedOut,
                    ..
                }
            }
        ));
        let inspected = service
            .handle(signed(&RunnerCommand::Inspect { run_id }, 11))
            .await;
        assert!(matches!(
            inspected.body,
            ResponseBody::Status {
                record: Some(RunRecord {
                    status: RunStatus::TimedOut,
                    ..
                })
            }
        ));
    }

    #[tokio::test]
    async fn cancellation_is_explicit_and_idempotent() {
        let service = service();
        let mut execution = request();
        execution.resources.timeout_seconds = 2;
        execution.simulation.delay_ms = 1_000;
        let run_id = execution.run_id;
        let running = {
            let service = Arc::clone(&service);
            tokio::spawn(async move {
                service
                    .handle(signed(
                        &RunnerCommand::Execute {
                            execution: Box::new(execution),
                        },
                        12,
                    ))
                    .await
            })
        };
        time::sleep(Duration::from_millis(20)).await;
        let cancel = service
            .handle(signed(&RunnerCommand::Cancel { run_id }, 13))
            .await;
        assert!(matches!(
            cancel.body,
            ResponseBody::CancelAcknowledged {
                already_terminal: false
            }
        ));
        let response = running.await.unwrap();
        assert!(matches!(
            response.body,
            ResponseBody::Finished {
                record: RunRecord {
                    status: RunStatus::Cancelled,
                    ..
                }
            }
        ));
        let repeated = service
            .handle(signed(&RunnerCommand::Cancel { run_id }, 14))
            .await;
        assert!(matches!(
            repeated.body,
            ResponseBody::CancelAcknowledged {
                already_terminal: true
            }
        ));
    }

    #[tokio::test]
    async fn framing_rejects_truncation_multiple_and_oversized_frames() {
        let (mut writer, mut reader) = tokio::io::duplex(MAX_FRAME_BYTES + 128);
        writer.write_all(b"{}\n{}\n").await.unwrap();
        assert!(matches!(
            read_frame(&mut reader).await,
            Err(FrameError::MultipleFrames)
        ));

        let (mut writer, mut reader) = tokio::io::duplex(16);
        writer.write_all(b"{}").await.unwrap();
        drop(writer);
        assert!(matches!(
            read_frame(&mut reader).await,
            Err(FrameError::Truncated)
        ));

        let (mut writer, mut reader) = tokio::io::duplex(MAX_FRAME_BYTES + 2);
        let write = tokio::spawn(async move {
            writer
                .write_all(&vec![b'a'; MAX_FRAME_BYTES + 1])
                .await
                .unwrap();
        });
        assert!(matches!(
            read_frame(&mut reader).await,
            Err(FrameError::TooLarge)
        ));
        write.await.unwrap();
    }

    #[tokio::test]
    async fn file_store_round_trips_and_uses_private_directory() {
        let directory = tempfile::tempdir().unwrap();
        let store = FileResultStore::new(directory.path().join("state"))
            .await
            .unwrap();
        let record = build_record(
            request(),
            unix_millis(),
            RunOutcome::Exited(BackendOutput {
                exit_code: 0,
                stdout: "done".into(),
                stderr: String::new(),
            }),
        );
        store.save(&record).await.unwrap();
        assert_eq!(
            store.load(record.request.run_id).await.unwrap(),
            Some(record)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(directory.path().join("state"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_created_privately_and_reused() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runner.key");
        let created = load_or_create_auth_key(&path).unwrap();
        assert_eq!(created.len(), 32);
        assert_eq!(load_or_create_auth_key(&path).unwrap(), created);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read(&path).unwrap(), created);
    }

    #[cfg(unix)]
    #[test]
    fn key_file_rejects_symlinks_and_insecure_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real.key");
        std::fs::write(&real, vec![42; 32]).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = directory.path().join("link.key");
        symlink(&real, &link).unwrap();
        assert!(matches!(
            load_or_create_auth_key(&link),
            Err(KeyFileError::UnsafeType)
        ));

        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            load_auth_key_file(&real),
            Err(KeyFileError::InsecurePermissions)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_key_creators_converge_without_partial_reads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runner.key");
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    load_or_create_auth_key(&path).unwrap()
                })
            })
            .collect();
        let keys: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert!(keys.iter().all(|key| key == &keys[0] && key.len() == 32));
        assert_eq!(
            std::fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1
        );
    }

    #[tokio::test]
    #[ignore = "requires Unix-socket operations that may be denied by build sandboxes"]
    async fn typed_client_round_trips_over_real_socket_and_rejects_wrong_key() {
        // macOS Unix-domain socket paths are limited to roughly 104 bytes;
        // tempfile's system path can exceed that before the file is created.
        let socket_path = PathBuf::from(format!("/tmp/c6-runner-test-{}.sock", Uuid::new_v4()));
        let daemon = tokio::spawn(serve(
            DaemonConfig {
                socket_path: socket_path.clone(),
            },
            service(),
        ));
        wait_for_socket(&socket_path).await;

        let client = Arc::new(RunnerClient::new(&socket_path, KEY).unwrap());
        assert!(matches!(client.ping().await.unwrap(), ResponseBody::Pong));

        let execution = request();
        let run_id = execution.run_id;
        assert!(matches!(
            client.execute(execution).await.unwrap(),
            ResponseBody::Finished {
                record: RunRecord {
                    status: RunStatus::Succeeded,
                    ..
                }
            }
        ));
        assert!(matches!(
            client.inspect(run_id).await.unwrap(),
            ResponseBody::Status { record: Some(_) }
        ));
        assert!(matches!(
            client.cancel(run_id).await.unwrap(),
            ResponseBody::CancelAcknowledged {
                already_terminal: true
            }
        ));

        let wrong_key = RunnerClient::new(
            &socket_path,
            b"wrong-key-that-is-still-at-least-32-bytes".to_vec(),
        )
        .unwrap();
        assert!(matches!(
            wrong_key.ping().await.unwrap(),
            ResponseBody::Rejected {
                code: ErrorCode::AuthenticationFailed,
                ..
            }
        ));

        daemon.abort();
        let _ = daemon.await;
    }

    #[cfg(unix)]
    #[test]
    fn socket_setup_refuses_regular_files_and_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let regular = directory.path().join("runner.sock");
        std::fs::write(&regular, b"do not delete").unwrap();
        assert!(matches!(
            prepare_socket_path(&regular),
            Err(DaemonError::UnsafeSocketPath)
        ));
        assert_eq!(std::fs::read(&regular).unwrap(), b"do not delete");

        let link = directory.path().join("link.sock");
        symlink(&regular, &link).unwrap();
        assert!(matches!(
            prepare_socket_path(&link),
            Err(DaemonError::UnsafeSocketPath)
        ));
        assert!(link.exists());
    }
}

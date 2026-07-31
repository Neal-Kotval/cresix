//! Bounded, read-only Git smart-HTTP CGI integration.
//!
//! This module deliberately exposes only `git-upload-pack`. Authorization and
//! public URL resolution belong to the server; the adapter receives an already
//! validated, store-owned [`Repository`] and never sees a workspace or project
//! slug. The current implementation buffers a bounded request and response. It
//! is suitable for the Phase 2.1 preview, but a streaming adapter is required
//! before raising the response limit to the 1 GiB product target.

use crate::Repository;
use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use thiserror::Error;

const ADVERTISEMENT_QUERY: &str = "service=git-upload-pack";
const UPLOAD_PACK_REQUEST: &str = "application/x-git-upload-pack-request";
const UPLOAD_PACK_ADVERTISEMENT: &str = "application/x-git-upload-pack-advertisement";
const UPLOAD_PACK_RESULT: &str = "application/x-git-upload-pack-result";

#[derive(Clone, Copy, Debug)]
pub struct SmartHttpLimits {
    /// `upload-pack` negotiation requests are small; pack data flows in the
    /// response. This also bounds the in-memory request buffer.
    pub max_request_bytes: usize,
    /// This adapter buffers output. Keep this substantially below the future
    /// streaming transport's limit.
    pub max_response_bytes: usize,
    pub max_header_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_git_protocol_bytes: usize,
    pub advertisement_timeout: Duration,
    pub upload_pack_timeout: Duration,
}

impl Default for SmartHttpLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: 16 * 1024 * 1024,
            max_response_bytes: 64 * 1024 * 1024,
            max_header_bytes: 32 * 1024,
            max_stderr_bytes: 16 * 1024,
            max_git_protocol_bytes: 256,
            advertisement_timeout: Duration::from_secs(30),
            upload_pack_timeout: Duration::from_secs(10 * 60),
        }
    }
}

#[derive(Debug, Error)]
pub enum SmartHttpError {
    #[error("invalid smart HTTP request: {0}")]
    InvalidRequest(&'static str),
    #[error("smart HTTP request exceeds the configured limit")]
    RequestTooLarge,
    #[error("smart HTTP response exceeds the configured limit")]
    ResponseTooLarge,
    #[error("smart HTTP child timed out")]
    Timeout,
    #[error("Git smart HTTP backend is unavailable")]
    Unavailable,
    #[error("Git smart HTTP backend failed")]
    BackendFailed,
    #[error("Git smart HTTP backend returned an invalid response")]
    InvalidResponse,
    #[error("repository is no longer a valid store-owned directory")]
    InvalidRepository,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmartHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct SmartHttpBackend {
    git_executable: PathBuf,
    child_path: OsString,
    limits: SmartHttpLimits,
}

impl SmartHttpBackend {
    /// Resolve `git` once from the operator's process environment. The child is
    /// subsequently launched by this canonical path with a cleared environment.
    pub fn from_environment(limits: SmartHttpLimits) -> Result<Self, SmartHttpError> {
        let path = env::var_os("PATH").ok_or(SmartHttpError::Unavailable)?;
        for directory in env::split_paths(&path) {
            let candidate = directory.join("git");
            if candidate.is_file() {
                return Self::with_git_executable(candidate, limits);
            }
        }
        Err(SmartHttpError::Unavailable)
    }

    /// Configure an explicit Git executable. This is an operator-controlled
    /// path, never request data. It must resolve to a regular file.
    pub fn with_git_executable(
        git_executable: impl AsRef<Path>,
        limits: SmartHttpLimits,
    ) -> Result<Self, SmartHttpError> {
        validate_limits(limits)?;
        let git_executable = git_executable
            .as_ref()
            .canonicalize()
            .map_err(|_| SmartHttpError::Unavailable)?;
        if !git_executable.is_file() {
            return Err(SmartHttpError::Unavailable);
        }
        let mut paths = vec![
            git_executable
                .parent()
                .ok_or(SmartHttpError::Unavailable)?
                .to_path_buf(),
        ];
        for trusted in [Path::new("/usr/bin"), Path::new("/bin")] {
            if !paths.iter().any(|path| path == trusted) {
                paths.push(trusted.to_path_buf());
            }
        }
        let child_path = env::join_paths(paths).map_err(|_| SmartHttpError::Unavailable)?;
        Ok(Self {
            git_executable,
            child_path,
            limits,
        })
    }

    /// Produce the `info/refs` advertisement for exactly `git-upload-pack`.
    pub fn advertise(
        &self,
        repository: &Repository,
        raw_query: &str,
        git_protocol: Option<&str>,
        actor_id: &str,
    ) -> Result<SmartHttpResponse, SmartHttpError> {
        if raw_query != ADVERTISEMENT_QUERY {
            return Err(SmartHttpError::InvalidRequest(
                "expected exactly service=git-upload-pack",
            ));
        }
        self.execute(
            repository,
            Request {
                method: "GET",
                path_suffix: "/info/refs",
                query: ADVERTISEMENT_QUERY,
                content_type: None,
                git_protocol,
                actor_id,
                body: &[],
                expected_content_type: UPLOAD_PACK_ADVERTISEMENT,
                timeout: self.limits.advertisement_timeout,
            },
        )
    }

    /// Run one read-only upload-pack negotiation. Query parameters are forbidden
    /// and the Git request media type must match exactly.
    pub fn upload_pack(
        &self,
        repository: &Repository,
        raw_query: Option<&str>,
        content_type: Option<&str>,
        git_protocol: Option<&str>,
        actor_id: &str,
        body: &[u8],
    ) -> Result<SmartHttpResponse, SmartHttpError> {
        if raw_query.is_some_and(|query| !query.is_empty()) {
            return Err(SmartHttpError::InvalidRequest(
                "upload-pack does not accept a query",
            ));
        }
        if content_type != Some(UPLOAD_PACK_REQUEST) {
            return Err(SmartHttpError::InvalidRequest(
                "invalid upload-pack content type",
            ));
        }
        if body.len() > self.limits.max_request_bytes {
            return Err(SmartHttpError::RequestTooLarge);
        }
        self.execute(
            repository,
            Request {
                method: "POST",
                path_suffix: "/git-upload-pack",
                query: "",
                content_type,
                git_protocol,
                actor_id,
                body,
                expected_content_type: UPLOAD_PACK_RESULT,
                timeout: self.limits.upload_pack_timeout,
            },
        )
    }

    fn execute(
        &self,
        repository: &Repository,
        request: Request<'_>,
    ) -> Result<SmartHttpResponse, SmartHttpError> {
        validate_actor(request.actor_id)?;
        validate_git_protocol(request.git_protocol, self.limits.max_git_protocol_bytes)?;
        let (project_root, repository_name) = validate_repository(repository)?;
        let private_home = TempDir::new().map_err(|_| SmartHttpError::Unavailable)?;
        let path_info = format!("/{repository_name}{}", request.path_suffix);

        let mut command = Command::new(&self.git_executable);
        command
            .arg("http-backend")
            .env_clear()
            .env("PATH", &self.child_path)
            .env("HOME", private_home.path())
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", private_home.path().join("gitconfig"))
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_PROJECT_ROOT", project_root)
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.getanyfile")
            .env("GIT_CONFIG_VALUE_0", "false")
            .env("REQUEST_METHOD", request.method)
            .env("PATH_INFO", path_info)
            .env("QUERY_STRING", request.query)
            .env("REMOTE_USER", request.actor_id)
            .env("CONTENT_LENGTH", request.body.len().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(content_type) = request.content_type {
            command.env("CONTENT_TYPE", content_type);
        }
        if let Some(protocol) = request.git_protocol {
            command.env("HTTP_GIT_PROTOCOL", protocol);
        }

        let mut child = command.spawn().map_err(|_| SmartHttpError::Unavailable)?;
        let stdin = child.stdin.take().ok_or(SmartHttpError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(SmartHttpError::Unavailable)?;
        let stderr = child.stderr.take().ok_or(SmartHttpError::Unavailable)?;
        let body = request.body.to_vec();
        let writer = thread::spawn(move || {
            let mut stdin = stdin;
            stdin.write_all(&body)
        });

        let overflow = Arc::new(AtomicBool::new(false));
        let stdout_overflow = Arc::clone(&overflow);
        let stdout_limit = self
            .limits
            .max_response_bytes
            .checked_add(self.limits.max_header_bytes)
            .ok_or(SmartHttpError::ResponseTooLarge)?;
        let stdout_reader =
            thread::spawn(move || read_bounded(stdout, stdout_limit, stdout_overflow));
        let stderr_reader = thread::spawn({
            let overflow = Arc::new(AtomicBool::new(false));
            let limit = self.limits.max_stderr_bytes;
            move || read_bounded(stderr, limit, overflow)
        });

        let started = Instant::now();
        let status = loop {
            if overflow.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                break Err(SmartHttpError::ResponseTooLarge);
            }
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if started.elapsed() < request.timeout => {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(SmartHttpError::Timeout);
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(SmartHttpError::BackendFailed);
                }
            }
        };

        let write_result = writer.join().map_err(|_| SmartHttpError::BackendFailed)?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| SmartHttpError::BackendFailed)??;
        let _bounded_stderr = stderr_reader
            .join()
            .map_err(|_| SmartHttpError::BackendFailed)??;
        let status = status?;
        if write_result.is_err() || !status.success() {
            return Err(SmartHttpError::BackendFailed);
        }
        parse_cgi_response(
            &stdout,
            self.limits.max_header_bytes,
            self.limits.max_response_bytes,
            request.expected_content_type,
        )
    }
}

struct Request<'a> {
    method: &'static str,
    path_suffix: &'static str,
    query: &'static str,
    content_type: Option<&'a str>,
    git_protocol: Option<&'a str>,
    actor_id: &'a str,
    body: &'a [u8],
    expected_content_type: &'static str,
    timeout: Duration,
}

fn validate_limits(limits: SmartHttpLimits) -> Result<(), SmartHttpError> {
    if limits.max_request_bytes == 0
        || limits.max_response_bytes == 0
        || limits.max_header_bytes < 128
        || limits.max_header_bytes > 1024 * 1024
        || limits.max_stderr_bytes == 0
        || limits.max_git_protocol_bytes == 0
        || limits.max_git_protocol_bytes > 4096
        || limits.advertisement_timeout.is_zero()
        || limits.upload_pack_timeout.is_zero()
    {
        return Err(SmartHttpError::InvalidRequest("unsafe smart HTTP limits"));
    }
    Ok(())
}

fn validate_actor(actor: &str) -> Result<(), SmartHttpError> {
    if actor.is_empty()
        || actor.len() > 128
        || !actor
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(SmartHttpError::InvalidRequest("invalid actor identifier"));
    }
    Ok(())
}

fn validate_git_protocol(protocol: Option<&str>, max: usize) -> Result<(), SmartHttpError> {
    let Some(protocol) = protocol else {
        return Ok(());
    };
    if protocol.len() > max {
        return Err(SmartHttpError::InvalidRequest("Git-Protocol is too large"));
    }
    // Phase 2.1 supports only Git's known version selector. Extensions are
    // denied until they receive an explicit security and compatibility review.
    if !matches!(protocol, "version=1" | "version=2") {
        return Err(SmartHttpError::InvalidRequest("unsupported Git-Protocol"));
    }
    Ok(())
}

fn validate_repository(repository: &Repository) -> Result<(PathBuf, String), SmartHttpError> {
    let metadata =
        fs::symlink_metadata(&repository.path).map_err(|_| SmartHttpError::InvalidRepository)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SmartHttpError::InvalidRepository);
    }
    let canonical = repository
        .path
        .canonicalize()
        .map_err(|_| SmartHttpError::InvalidRepository)?;
    if canonical != repository.path {
        return Err(SmartHttpError::InvalidRepository);
    }
    let root = canonical
        .parent()
        .ok_or(SmartHttpError::InvalidRepository)?;
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(SmartHttpError::InvalidRepository)?;
    let key = name
        .strip_suffix(".git")
        .ok_or(SmartHttpError::InvalidRepository)?;
    crate::validate_slug(key).map_err(|_| SmartHttpError::InvalidRepository)?;
    Ok((root.to_path_buf(), name.to_owned()))
}

fn read_bounded<R: Read>(
    mut reader: R,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> Result<Vec<u8>, SmartHttpError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        let read = reader
            .read(&mut chunk)
            .map_err(|_| SmartHttpError::BackendFailed)?;
        if read == 0 {
            return Ok(bytes);
        }
        if bytes.len().saturating_add(read) > limit {
            overflow.store(true, Ordering::Relaxed);
        } else if !overflow.load(Ordering::Relaxed) {
            bytes.extend_from_slice(&chunk[..read]);
        }
    }
}

fn parse_cgi_response(
    bytes: &[u8],
    max_header_bytes: usize,
    max_body_bytes: usize,
    expected_content_type: &str,
) -> Result<SmartHttpResponse, SmartHttpError> {
    let separator = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| (position, 4))
        .or_else(|| {
            bytes
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|position| (position, 2))
        })
        .ok_or(SmartHttpError::InvalidResponse)?;
    if separator.0.saturating_add(separator.1) > max_header_bytes {
        return Err(SmartHttpError::InvalidResponse);
    }
    let body = &bytes[separator.0 + separator.1..];
    if body.len() > max_body_bytes {
        return Err(SmartHttpError::ResponseTooLarge);
    }
    let header_text =
        std::str::from_utf8(&bytes[..separator.0]).map_err(|_| SmartHttpError::InvalidResponse)?;
    let mut status = 200;
    let mut saw_status = false;
    let mut saw_content_type = false;
    let mut headers = Vec::new();
    for line in header_text.lines() {
        if line.is_empty() || line.starts_with([' ', '\t']) || line.bytes().any(|b| b == 0) {
            return Err(SmartHttpError::InvalidResponse);
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(SmartHttpError::InvalidResponse)?;
        let value = value.trim_matches([' ', '\t']);
        if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(SmartHttpError::InvalidResponse);
        }
        match name.to_ascii_lowercase().as_str() {
            "status" => {
                if saw_status {
                    return Err(SmartHttpError::InvalidResponse);
                }
                saw_status = true;
                let code = value
                    .split_ascii_whitespace()
                    .next()
                    .ok_or(SmartHttpError::InvalidResponse)?
                    .parse::<u16>()
                    .map_err(|_| SmartHttpError::InvalidResponse)?;
                if !(200..=599).contains(&code) {
                    return Err(SmartHttpError::InvalidResponse);
                }
                status = code;
            }
            "content-type" => {
                if saw_content_type || value != expected_content_type {
                    return Err(SmartHttpError::InvalidResponse);
                }
                saw_content_type = true;
                headers.push(("Content-Type".into(), value.into()));
            }
            "cache-control" => headers.push(("Cache-Control".into(), value.into())),
            "expires" => headers.push(("Expires".into(), value.into())),
            "pragma" => headers.push(("Pragma".into(), value.into())),
            // Unknown CGI-controlled headers are intentionally discarded. They
            // cannot influence framing, cookies, redirects, or CORS.
            _ => {}
        }
    }
    if !saw_content_type {
        return Err(SmartHttpError::InvalidResponse);
    }
    Ok(SmartHttpResponse {
        status,
        headers,
        body: body.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommitIdentity, FileChange, GitStore};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn git_path() -> PathBuf {
        let path = env::var_os("PATH").unwrap();
        env::split_paths(&path)
            .map(|directory| directory.join("git"))
            .find(|candidate| candidate.is_file())
            .expect("tests require Git")
    }

    fn fixture() -> (tempfile::TempDir, Repository, SmartHttpBackend) {
        let temp = tempfile::tempdir().unwrap();
        let store = GitStore::new(temp.path().join("repos")).unwrap();
        let repository = store
            .create("3f40e12a-452d-4e73-b608-b1f1d735cb20")
            .unwrap();
        repository
            .commit_changes(
                "main",
                None,
                &[FileChange::Upsert {
                    path: "README.md".into(),
                    content: b"hello\n".to_vec(),
                }],
                "initial",
                &CommitIdentity {
                    name: "C6 Test".into(),
                    email: "test@c6.local".into(),
                },
            )
            .unwrap();
        let backend =
            SmartHttpBackend::with_git_executable(git_path(), SmartHttpLimits::default()).unwrap();
        (temp, repository, backend)
    }

    #[test]
    fn advertises_an_owned_repository_without_public_slugs() {
        let (_temp, repository, backend) = fixture();
        let response = backend
            .advertise(
                &repository,
                "service=git-upload-pack",
                Some("version=2"),
                "user-123",
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert!(response.headers.contains(&(
            "Content-Type".into(),
            "application/x-git-upload-pack-advertisement".into()
        )));
        assert!(response.body.starts_with(b"000eversion 2\n"));
        assert!(!String::from_utf8_lossy(&response.body).contains("user-123"));
    }

    #[test]
    fn serves_a_bounded_protocol_v2_upload_pack_rpc() {
        let (_temp, repository, backend) = fixture();
        // `ls-refs` has no pack response, making it a compact end-to-end CGI
        // fixture while still exercising upload-pack stdin and stdout.
        let response = backend
            .upload_pack(
                &repository,
                None,
                Some(UPLOAD_PACK_REQUEST),
                Some("version=2"),
                "user-123",
                b"0014command=ls-refs\n0001000csymrefs\n0009peel\n0000",
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert!(
            response
                .headers
                .contains(&("Content-Type".into(), UPLOAD_PACK_RESULT.into()))
        );
        assert!(String::from_utf8_lossy(&response.body).contains("refs/heads/main"));
    }

    #[test]
    fn denies_every_service_except_exact_upload_pack_metadata() {
        let (_temp, repository, backend) = fixture();
        for query in [
            "",
            "service=git-receive-pack",
            "service=git-upload-pack&x=1",
            "x=1&service=git-upload-pack",
            "service=git%2dupload-pack",
        ] {
            assert!(matches!(
                backend.advertise(&repository, query, None, "user-1"),
                Err(SmartHttpError::InvalidRequest(_))
            ));
        }
        assert!(matches!(
            backend.upload_pack(
                &repository,
                None,
                Some("application/x-git-receive-pack-request"),
                None,
                "user-1",
                b"0000"
            ),
            Err(SmartHttpError::InvalidRequest(_))
        ));
        assert!(matches!(
            backend.upload_pack(
                &repository,
                Some("service=git-upload-pack"),
                Some(UPLOAD_PACK_REQUEST),
                None,
                "user-1",
                b"0000"
            ),
            Err(SmartHttpError::InvalidRequest(_))
        ));
    }

    #[test]
    fn rejects_protocol_injection_actor_injection_and_oversized_input_before_spawn() {
        let (_temp, repository, backend) = fixture();
        for protocol in ["version=3", "version=2\nEVIL=1", "version=2:agent=x"] {
            assert!(matches!(
                backend.advertise(&repository, ADVERTISEMENT_QUERY, Some(protocol), "user-1"),
                Err(SmartHttpError::InvalidRequest(_))
            ));
        }
        assert!(matches!(
            backend.advertise(
                &repository,
                ADVERTISEMENT_QUERY,
                None,
                "user\nREMOTE_USER=admin"
            ),
            Err(SmartHttpError::InvalidRequest(_))
        ));
        let limits = SmartHttpLimits {
            max_request_bytes: 3,
            ..SmartHttpLimits::default()
        };
        let backend = SmartHttpBackend::with_git_executable(git_path(), limits).unwrap();
        assert!(matches!(
            backend.upload_pack(
                &repository,
                None,
                Some(UPLOAD_PACK_REQUEST),
                None,
                "user-1",
                b"0000"
            ),
            Err(SmartHttpError::RequestTooLarge)
        ));
    }

    #[test]
    fn cgi_parser_allows_only_safe_headers_and_exact_media_type() {
        let response = parse_cgi_response(
            b"Status: 200 OK\r\nContent-Type: application/x-git-upload-pack-result\r\nSet-Cookie: stolen=yes\r\nLocation: https://evil.example\r\nX-Test: ignored\r\n\r\n0000",
            1024,
            1024,
            UPLOAD_PACK_RESULT,
        )
        .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers,
            vec![("Content-Type".into(), UPLOAD_PACK_RESULT.into())]
        );
        assert_eq!(response.body, b"0000");

        for malformed in [
            b"Content-Type: text/html\r\n\r\nx".as_slice(),
            b"Content-Type: application/x-git-upload-pack-result\r\n Content-Length: 1\r\n\r\nx",
            b"Status: 700 Bad\r\nContent-Type: application/x-git-upload-pack-result\r\n\r\nx",
            b"Content-Type: application/x-git-upload-pack-result\r\nContent-Type: application/x-git-upload-pack-result\r\n\r\nx",
        ] {
            assert!(matches!(
                parse_cgi_response(malformed, 1024, 1024, UPLOAD_PACK_RESULT),
                Err(SmartHttpError::InvalidResponse)
            ));
        }

        let oversized_headers = format!(
            "X-Ignored: {}\r\nContent-Type: {UPLOAD_PACK_RESULT}\r\n\r\nx",
            "a".repeat(200)
        );
        assert!(matches!(
            parse_cgi_response(oversized_headers.as_bytes(), 128, 1024, UPLOAD_PACK_RESULT),
            Err(SmartHttpError::InvalidResponse)
        ));
        assert!(matches!(
            parse_cgi_response(
                b"Content-Type: application/x-git-upload-pack-result\r\n\r\ntoo large",
                1024,
                3,
                UPLOAD_PACK_RESULT
            ),
            Err(SmartHttpError::ResponseTooLarge)
        ));
    }

    #[test]
    fn bounded_reader_discards_overflow_instead_of_growing_the_buffer() {
        let overflow = Arc::new(AtomicBool::new(false));
        let bytes = read_bounded(&b"0123456789"[..], 4, Arc::clone(&overflow)).unwrap();
        assert!(overflow.load(Ordering::Relaxed));
        assert!(bytes.len() <= 4);
    }

    #[test]
    fn rejects_repository_replaced_by_symlink() {
        let (temp, repository, backend) = fixture();
        #[cfg(unix)]
        {
            let path = repository.path.clone();
            fs::rename(&path, temp.path().join("moved.git")).unwrap();
            std::os::unix::fs::symlink(temp.path().join("moved.git"), &path).unwrap();
            assert!(matches!(
                backend.advertise(&repository, ADVERTISEMENT_QUERY, None, "user-1"),
                Err(SmartHttpError::InvalidRepository)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn kills_and_reaps_a_backend_that_exceeds_its_deadline() {
        let (temp, repository, _backend) = fixture();
        let fake_git = temp.path().join("fake-git");
        fs::write(&fake_git, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
        fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o700)).unwrap();
        let limits = SmartHttpLimits {
            advertisement_timeout: Duration::from_millis(20),
            ..SmartHttpLimits::default()
        };
        let backend = SmartHttpBackend::with_git_executable(&fake_git, limits).unwrap();
        let started = Instant::now();
        assert!(matches!(
            backend.advertise(&repository, ADVERTISEMENT_QUERY, None, "user-1"),
            Err(SmartHttpError::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn drains_but_does_not_expose_or_unboundedly_capture_child_stderr() {
        let (temp, repository, _backend) = fixture();
        let fake_git = temp.path().join("fake-git");
        fs::write(
            &fake_git,
            "#!/bin/sh\ni=0\nwhile [ $i -lt 1000 ]; do printf x >&2; i=$((i+1)); done\nprintf 'Content-Type: application/x-git-upload-pack-advertisement\\r\\n\\r\\n0000'\n",
        )
        .unwrap();
        fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o700)).unwrap();
        let limits = SmartHttpLimits {
            max_stderr_bytes: 4,
            ..SmartHttpLimits::default()
        };
        let backend = SmartHttpBackend::with_git_executable(&fake_git, limits).unwrap();
        let response = backend
            .advertise(&repository, ADVERTISEMENT_QUERY, None, "user-1")
            .unwrap();
        assert_eq!(response.body, b"0000");
        assert!(!format!("{response:?}").contains(&"x".repeat(10)));
    }
}

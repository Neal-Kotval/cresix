//! Single-node Cresix Cloud dogfood authority.
//!
//! This crate deliberately does not pretend to be a production identity provider.
//! Its bootstrap account is claimed from loopback, and it provides the smallest
//! account/workspace/installation boundary needed to dogfood connected C6 installs.

use std::{
    collections::{HashMap, HashSet},
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, bail};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{
        ConnectInfo, DefaultBodyLimit, Path as AxumPath, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, HeaderValue, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, delete, get, post, put},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use c6_cloud_core::{
    AccountSummary, BootstrapClaimRequest, BootstrapClaimResponse, CatalogAcceptedResponse,
    CloudSessionResponse, CloudStatusResponse, CloudWorkspaceDirectorySummary,
    CloudWorkspaceListResponse, CloudWorkspaceRole, CloudWorkspaceSummary,
    CreateCloudWorkspaceRequest, CreateWorkspaceBindingRequest, DirectoryProjectResponse,
    HeaderField, HttpMethod, InstallationConnectionState, InstallationListResponse,
    InstallationSummary, MAX_BODY_CHUNK_BYTES, MAX_REQUEST_BODY_BYTES, MAX_RESPONSE_BODY_BYTES,
    PutCatalogRequest, RELAY_SUBPROTOCOL, RegisterInstallationRequest,
    RegisterInstallationResponse, RelayBodyFrame, RelayBodyKind, RelayControlFrame,
    RelayHeartbeatResponse, RelaySessionState, RequestIdFrame, RequestStartFrame,
    RevokeInstallationResponse, SecretToken, ServerReadyFrame, TokenClass, WorkspaceBindingSummary,
    WorkspaceNamespace,
};
use chrono::{Duration, Utc};
use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::{mpsc, oneshot};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use uuid::Uuid;

const SESSION_COOKIE: &str = "c6_cloud_session";
const SECURE_SESSION_COOKIE: &str = "__Host-c6_cloud_session";
const CSRF_COOKIE: &str = "c6_cloud_csrf";
const SECURE_CSRF_COOKIE: &str = "__Host-c6_cloud_csrf";
const SESSION_HOURS: i64 = 24 * 30;
const MAX_BODY: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub public_origin: String,
    pub web_dir: PathBuf,
}

#[derive(Clone)]
pub struct Cloud {
    db: Arc<Mutex<Connection>>,
    config: Config,
    secure_cookies: bool,
    bootstrap_path: PathBuf,
    relays: Arc<tokio::sync::Mutex<HashMap<String, RelayRegistration>>>,
}

#[derive(Clone)]
struct RelayRegistration {
    session_id: Uuid,
    sender: mpsc::Sender<ProxyExchange>,
}

struct ProxyExchange {
    method: String,
    target: String,
    headers: Vec<HeaderField>,
    body: Vec<u8>,
    deadline: tokio::time::Instant,
    reply: oneshot::Sender<Result<ProxyResult, &'static str>>,
}

struct ProxyResult {
    status: u16,
    headers: Vec<HeaderField>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    request_id: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            request_id: Uuid::new_v4().to_string(),
        }
    }
    fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }
    fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "authentication required",
        )
    }
    fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }
    fn not_found(kind: &'static str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{kind} not found"),
        )
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }
    fn internal(error: impl std::fmt::Display) -> Self {
        let request_id = Uuid::new_v4().to_string();
        tracing::error!(%request_id, %error, "cloud request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "internal server error".into(),
            request_id,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "code": self.code, "message": self.message, "requestId": self.request_id
            })),
        )
            .into_response()
    }
}

#[derive(Debug, Clone)]
struct UserPrincipal {
    account_id: String,
    csrf_hash: String,
}

#[derive(Debug, Clone)]
struct ConnectorPrincipal {
    installation_id: String,
}

pub fn app(cloud: Cloud) -> Router {
    let web_dir = cloud.config.web_dir.clone();
    let index = web_dir.join("index.html");
    let api = Router::new()
        .route("/status", get(status))
        .route("/bootstrap/claim", post(claim_bootstrap))
        .route("/session", get(session))
        .route("/session", delete(logout))
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route(
            "/installations",
            get(list_installations).post(register_installation),
        )
        .route("/workspaces/{id}/binding", post(bind_installation))
        .route("/workspaces/{id}/bindings", post(bind_installation))
        .route("/installations/{id}", delete(revoke_installation))
        .route("/installations/{id}/catalog", put(put_catalog))
        .route("/installations/{id}/heartbeat", post(heartbeat))
        .route("/directory/{workspace}/{project}", get(directory_project))
        .route("/relay/connect", get(relay_connect))
        .fallback(api_not_found);

    Router::new()
        .route("/healthz", get(|| async { Json(json!({"status":"ok"})) }))
        .nest("/api/v1", api)
        .route("/relay/{route}/{*path}", any(relay_proxy))
        .fallback_service(ServeDir::new(web_dir).fallback(ServeFile::new(index)))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(middleware::from_fn(security_headers))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                // Paths, queries, headers, and bodies are deliberately absent:
                // all are attacker-controlled and may contain credentials.
                tracing::info_span!("cloud_http_request", method = %request.method())
            }),
        )
        .with_state(cloud)
}

async fn api_not_found() -> ApiError {
    ApiError::not_found("API route")
}

async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let sensitive = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' ws: wss:",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    if sensitive {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

impl Cloud {
    pub fn open(config: Config) -> anyhow::Result<Self> {
        validate_origin(&config.public_origin)?;
        prepare_data_dir(&config.data_dir)?;
        let mut conn = Connection::open(config.data_dir.join("cloud.sqlite3"))
            .context("open cloud database")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        // Relay presence is process-local. A prior clean/unclean shutdown can
        // never be evidence that an outbound connector is live in this process.
        conn.execute(
            "UPDATE installations SET connected_at=NULL WHERE revoked_at IS NULL",
            [],
        )?;
        let bootstrap_path = config.data_dir.join("bootstrap-token");
        ensure_bootstrap(&mut conn, &bootstrap_path)?;
        let secure_cookies = config.public_origin.starts_with("https://");
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            config,
            secure_cookies,
            bootstrap_path,
            relays: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    pub fn bootstrap_token_path(&self) -> &Path {
        &self.bootstrap_path
    }
}

fn prepare_data_dir(data_dir: &Path) -> anyhow::Result<()> {
    if data_dir.as_os_str().is_empty() || data_dir.parent().is_none() {
        bail!("C6_CLOUD_DATA_DIR must be a dedicated directory, not a filesystem root");
    }
    let created = match fs::symlink_metadata(data_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            bail!("C6_CLOUD_DATA_DIR must be a real directory, not a symlink or file")
        }
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(data_dir).context("create cloud data directory")?;
            true
        }
        Err(error) => return Err(error).context("inspect cloud data directory"),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if created {
            fs::set_permissions(data_dir, fs::Permissions::from_mode(0o700))
                .context("secure C6_CLOUD_DATA_DIR permissions")?;
        } else if fs::metadata(data_dir)
            .context("inspect C6_CLOUD_DATA_DIR permissions")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            bail!("existing C6_CLOUD_DATA_DIR must have owner-only permissions (mode 0700)");
        }
    }
    Ok(())
}

fn validate_origin(origin: &str) -> anyhow::Result<()> {
    let rest = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
        .context("C6_CLOUD_PUBLIC_ORIGIN must be an absolute HTTP(S) origin")?;
    if rest.is_empty()
        || rest.contains('/')
        || rest.contains('?')
        || rest.contains('#')
        || rest.contains('@')
    {
        bail!("C6_CLOUD_PUBLIC_ORIGIN must contain only scheme and authority");
    }
    Ok(())
}

fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    conn.execute_batch(r#"
    BEGIN IMMEDIATE;
    CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS accounts(
        id TEXT PRIMARY KEY, handle TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL,
        created_at TEXT NOT NULL, disabled_at TEXT
    );
    CREATE TABLE IF NOT EXISTS sessions(
        id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id),
        token_hash TEXT NOT NULL UNIQUE, csrf_hash TEXT NOT NULL,
        created_at TEXT NOT NULL, expires_at TEXT NOT NULL, revoked_at TEXT
    );
    CREATE TABLE IF NOT EXISTS workspaces(
        id TEXT PRIMARY KEY, namespace TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL,
        owner_account_id TEXT NOT NULL REFERENCES accounts(id), created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS memberships(
        workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        account_id TEXT NOT NULL REFERENCES accounts(id), role TEXT NOT NULL,
        PRIMARY KEY(workspace_id, account_id)
    );
    CREATE TABLE IF NOT EXISTS installations(
        id TEXT PRIMARY KEY, owner_account_id TEXT NOT NULL REFERENCES accounts(id),
        local_server_id TEXT NOT NULL, route_id TEXT NOT NULL UNIQUE, label TEXT NOT NULL,
        catalog_generation INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL,
        connected_at TEXT, revoked_at TEXT, UNIQUE(owner_account_id, local_server_id)
    );
    CREATE TABLE IF NOT EXISTS connector_credentials(
        id TEXT PRIMARY KEY, installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
        public_id TEXT NOT NULL UNIQUE, secret_hash TEXT NOT NULL,
        created_at TEXT NOT NULL, revoked_at TEXT
    );
    CREATE TABLE IF NOT EXISTS catalog_projects(
        binding_id TEXT NOT NULL REFERENCES workspace_bindings(id) ON DELETE CASCADE,
        local_project_id TEXT NOT NULL, slug TEXT NOT NULL, name TEXT NOT NULL,
        description TEXT NOT NULL, default_branch TEXT NOT NULL, head_sha TEXT NOT NULL,
        revision INTEGER NOT NULL, updated_at TEXT NOT NULL,
        PRIMARY KEY(binding_id, slug), UNIQUE(binding_id, local_project_id)
    );
    CREATE TABLE IF NOT EXISTS workspace_bindings(
        id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL UNIQUE REFERENCES workspaces(id) ON DELETE CASCADE,
        installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
        local_workspace_id TEXT NOT NULL, catalog_revision INTEGER NOT NULL DEFAULT 0,
        UNIQUE(installation_id, local_workspace_id)
    );
    CREATE TABLE IF NOT EXISTS audit_events(
        id TEXT PRIMARY KEY, actor_type TEXT NOT NULL, actor_id TEXT,
        action TEXT NOT NULL, target_id TEXT, created_at TEXT NOT NULL
    );
    COMMIT;
    "#).context("migrate cloud database")?;
    Ok(())
}

fn ensure_bootstrap(conn: &mut Connection, path: &Path) -> anyhow::Result<()> {
    let claimed: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key='bootstrap_claimed'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if claimed.as_deref() == Some("1") {
        return Ok(());
    }

    let existing_hash: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key='bootstrap_hash'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if existing_hash.is_none() {
        let token = issue_token(TokenClass::Bootstrap);
        conn.execute(
            "INSERT INTO settings(key,value) VALUES('bootstrap_hash',?1)",
            [hash(token.expose_secret())],
        )?;
        write_private(path, &format!("{}\n", token.expose_secret()))?;
    } else if !path.exists() {
        // A missing one-time bootstrap file is not regenerated because doing so
        // would replace the credential represented by the persisted verifier.
        tracing::warn!(path=%path.display(), "unclaimed bootstrap credential file is missing");
    }
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, value: &str) -> anyhow::Result<()> {
    use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .context("create bootstrap credential")?;
    file.write_all(value.as_bytes())
        .context("write bootstrap credential")
}

#[cfg(not(unix))]
fn write_private(path: &Path, value: &str) -> anyhow::Result<()> {
    fs::write(path, value).context("write bootstrap credential")
}

async fn status(State(cloud): State<Cloud>) -> Result<Json<CloudStatusResponse>, ApiError> {
    let db = lock(&cloud)?;
    let claimed: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key='bootstrap_claimed' AND value='1')",
            [],
            |row| row.get(0),
        )
        .map_err(ApiError::internal)?;
    Ok(Json(CloudStatusResponse {
        claimed,
        service_name: "Cresix Cloud (dogfood)".into(),
        relay_authority: cloud.config.public_origin.clone(),
    }))
}

#[axum::debug_handler]
async fn claim_bootstrap(
    State(cloud): State<Cloud>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<BootstrapClaimRequest>,
) -> Result<Response, ApiError> {
    if !peer.ip().is_loopback() {
        return Err(ApiError::forbidden(
            "bootstrap claim is available from loopback only",
        ));
    }
    require_origin(&cloud, &headers)?;
    input
        .validate()
        .map_err(|error| ApiError::bad(error.to_string()))?;

    let mut db = lock(&cloud)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let claimed: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key='bootstrap_claimed' AND value='1')",
            [],
            |row| row.get(0),
        )
        .map_err(ApiError::internal)?;
    if claimed {
        return Err(ApiError::conflict(
            "cloud bootstrap has already been claimed",
        ));
    }
    let expected: String = tx
        .query_row(
            "SELECT value FROM settings WHERE key='bootstrap_hash'",
            [],
            |row| row.get(0),
        )
        .map_err(ApiError::internal)?;
    if input.bootstrap_token.parsed().class != TokenClass::Bootstrap
        || !constant_eq(&expected, &hash(input.bootstrap_token.expose_secret()))
    {
        return Err(ApiError::forbidden("invalid bootstrap credential"));
    }

    let now = Utc::now();
    let account_uuid = Uuid::new_v4();
    let account_id = account_uuid.to_string();
    tx.execute(
        "INSERT INTO accounts(id,handle,display_name,created_at) VALUES(?1,?2,?3,?4)",
        params![
            account_id,
            input.handle.as_str(),
            input.display_name,
            now.to_rfc3339()
        ],
    )
    .map_err(map_conflict)?;
    tx.execute("INSERT INTO settings(key,value) VALUES('bootstrap_claimed','1') ON CONFLICT(key) DO UPDATE SET value='1'", [])
        .map_err(ApiError::internal)?;
    tx.execute("DELETE FROM settings WHERE key='bootstrap_hash'", [])
        .map_err(ApiError::internal)?;
    let (session_token, csrf, session_id) = create_session(&tx, &account_id)?;
    audit(
        &tx,
        "account",
        Some(&account_id),
        "cloud.bootstrap_claimed",
        Some(&account_id),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    drop(db);
    if let Err(error) = fs::remove_file(&cloud.bootstrap_path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "could not remove consumed bootstrap file");
    }
    let _ = session_id;
    session_response(
        &cloud,
        &session_token,
        &csrf,
        BootstrapClaimResponse {
            account: AccountSummary {
                id: account_uuid,
                handle: input.handle,
                display_name: input.display_name,
                created_at: now,
                disabled_at: None,
            },
            csrf_token: SecretToken::parse(csrf.clone()).map_err(ApiError::internal)?,
        },
    )
}

async fn session(
    State(cloud): State<Cloud>,
    headers: HeaderMap,
) -> Result<Json<CloudSessionResponse>, ApiError> {
    let principal = user(&cloud, &headers, false)?;
    let db = lock(&cloud)?;
    let (handle, display_name, created_at, expires_at): (String, String, String, String) = db.query_row(
        "SELECT a.handle,a.display_name,a.created_at,s.expires_at FROM accounts a JOIN sessions s ON s.account_id=a.id WHERE a.id=?1 AND s.token_hash=?2",
        params![principal.account_id, hash(&cookie(&headers, session_cookie_name(&cloud)).ok_or_else(ApiError::unauthenticated)?)],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(ApiError::internal)?;
    let csrf = cookie(&headers, csrf_cookie_name(&cloud)).ok_or_else(ApiError::unauthenticated)?;
    Ok(Json(CloudSessionResponse {
        account: AccountSummary {
            id: parse_uuid(&principal.account_id)?,
            handle: handle.parse().map_err(ApiError::internal)?,
            display_name,
            created_at: parse_time(&created_at)?,
            disabled_at: None,
        },
        csrf_token: SecretToken::parse(csrf).map_err(ApiError::internal)?,
        expires_at: parse_time(&expires_at)?,
    }))
}

async fn logout(State(cloud): State<Cloud>, headers: HeaderMap) -> Result<Response, ApiError> {
    let principal = user(&cloud, &headers, true)?;
    let db = lock(&cloud)?;
    let token =
        cookie(&headers, session_cookie_name(&cloud)).ok_or_else(ApiError::unauthenticated)?;
    db.execute(
        "UPDATE sessions SET revoked_at=?1 WHERE account_id=?2 AND token_hash=?3",
        params![Utc::now().to_rfc3339(), principal.account_id, hash(&token)],
    )
    .map_err(ApiError::internal)?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_clear_cookies(&cloud, response.headers_mut());
    Ok(response)
}

async fn list_workspaces(
    State(cloud): State<Cloud>,
    headers: HeaderMap,
) -> Result<Json<CloudWorkspaceListResponse>, ApiError> {
    let principal = user(&cloud, &headers, false)?;
    let db = lock(&cloud)?;
    let mut statement = db.prepare(
        "SELECT w.id,w.namespace,w.display_name,w.owner_account_id,m.role,w.created_at FROM workspaces w JOIN memberships m ON m.workspace_id=w.id WHERE m.account_id=?1 ORDER BY w.namespace"
    ).map_err(ApiError::internal)?;
    let raw = statement
        .query_map([principal.account_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(ApiError::internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::internal)?;
    let summaries = raw
        .into_iter()
        .map(|(id, namespace, name, owner, role, created)| {
            Ok(CloudWorkspaceSummary {
                id: parse_uuid(&id)?,
                namespace: namespace.parse().map_err(ApiError::internal)?,
                name,
                owner_account_id: parse_uuid(&owner)?,
                role: parse_role(&role)?,
                created_at: parse_time(&created)?,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let mut workspaces = Vec::with_capacity(summaries.len());
    for workspace in summaries {
        let binding_raw: Option<(String, String, String, i64)> = db
            .query_row(
                "SELECT id,installation_id,local_workspace_id,catalog_revision FROM workspace_bindings WHERE workspace_id=?1",
                [workspace.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(ApiError::internal)?;
        let binding = binding_raw
            .map(
                |(id, installation_id, local_workspace_id, catalog_revision)| {
                    Ok(WorkspaceBindingSummary {
                        id: parse_uuid(&id)?,
                        workspace_id: workspace.id,
                        installation_id: parse_uuid(&installation_id)?,
                        local_workspace_id: parse_uuid(&local_workspace_id)?,
                        catalog_revision: u64::try_from(catalog_revision)
                            .map_err(ApiError::internal)?,
                    })
                },
            )
            .transpose()?;
        let projects = if let Some(binding) = &binding {
            let mut projects_statement = db
                .prepare(
                    "SELECT local_project_id,slug,name,description,default_branch,head_sha,updated_at FROM catalog_projects WHERE binding_id=?1 ORDER BY name,slug",
                )
                .map_err(ApiError::internal)?;
            let rows = projects_statement
                .query_map([binding.id.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                })
                .map_err(ApiError::internal)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(ApiError::internal)?;
            rows.into_iter()
                .map(
                    |(
                        local_project_id,
                        slug,
                        name,
                        description,
                        default_branch,
                        head_sha,
                        updated_at,
                    )| {
                        Ok(c6_cloud_core::CatalogProject {
                            binding_id: binding.id,
                            local_project_id: parse_uuid(&local_project_id)?,
                            slug: slug.parse().map_err(ApiError::internal)?,
                            name,
                            description,
                            default_branch,
                            head_sha,
                            updated_at: parse_time(&updated_at)?,
                        })
                    },
                )
                .collect::<Result<Vec<_>, ApiError>>()?
        } else {
            Vec::new()
        };
        workspaces.push(CloudWorkspaceDirectorySummary {
            workspace,
            binding,
            projects,
        });
    }
    Ok(Json(CloudWorkspaceListResponse { workspaces }))
}

async fn create_workspace(
    State(cloud): State<Cloud>,
    headers: HeaderMap,
    Json(input): Json<CreateCloudWorkspaceRequest>,
) -> Result<(StatusCode, Json<CloudWorkspaceSummary>), ApiError> {
    let principal = user(&cloud, &headers, true)?;
    input
        .validate()
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let now = Utc::now();
    let workspace_uuid = Uuid::new_v4();
    let id = workspace_uuid.to_string();
    let mut db = lock(&cloud)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute("INSERT INTO workspaces(id,namespace,display_name,owner_account_id,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![id,input.namespace.as_str(),input.name,principal.account_id,now.to_rfc3339()]).map_err(map_conflict)?;
    tx.execute(
        "INSERT INTO memberships(workspace_id,account_id,role) VALUES(?1,?2,'owner')",
        params![id, principal.account_id],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        "account",
        Some(&principal.account_id),
        "workspace.created",
        Some(&id),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(CloudWorkspaceSummary {
            id: workspace_uuid,
            namespace: input.namespace,
            name: input.name,
            owner_account_id: parse_uuid(&principal.account_id)?,
            role: CloudWorkspaceRole::Owner,
            created_at: now,
        }),
    ))
}

async fn register_installation(
    State(cloud): State<Cloud>,
    headers: HeaderMap,
    Json(input): Json<RegisterInstallationRequest>,
) -> Result<(StatusCode, Json<RegisterInstallationResponse>), ApiError> {
    let principal = user(&cloud, &headers, true)?;
    let installation_uuid = Uuid::new_v4();
    let installation_id = installation_uuid.to_string();
    let credential_id = Uuid::new_v4().to_string();
    let connector_token = issue_token(TokenClass::Connector);
    let parsed = connector_token.parsed();
    let public_id = parsed.public_id.to_owned();
    let secret_hash = hash(parsed.expose_proof());
    let route_id = short_id();
    let now = Utc::now();
    let mut db = lock(&cloud)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute("INSERT INTO installations(id,owner_account_id,local_server_id,route_id,label,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![installation_id,principal.account_id,input.local_server_id.to_string(),route_id,input.label.as_str(),now.to_rfc3339()]).map_err(map_conflict)?;
    tx.execute("INSERT INTO connector_credentials(id,installation_id,public_id,secret_hash,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![credential_id,installation_id,public_id,secret_hash,now.to_rfc3339()]).map_err(ApiError::internal)?;
    audit(
        &tx,
        "account",
        Some(&principal.account_id),
        "installation.registered",
        Some(&installation_id),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(RegisterInstallationResponse {
            installation: InstallationSummary {
                id: installation_uuid,
                local_server_id: input.local_server_id,
                route_id,
                owner_account_id: parse_uuid(&principal.account_id)?,
                label: input.label,
                credential_public_id: public_id,
                connection_state: InstallationConnectionState::Disconnected,
                connected_at: None,
                created_at: now,
                revoked_at: None,
            },
            connector_token,
        }),
    ))
}

async fn list_installations(
    State(cloud): State<Cloud>,
    headers: HeaderMap,
) -> Result<Json<InstallationListResponse>, ApiError> {
    let principal = user(&cloud, &headers, false)?;
    let db = lock(&cloud)?;
    let mut statement = db.prepare(r#"SELECT i.id,i.local_server_id,i.route_id,i.owner_account_id,i.label,c.public_id,i.connected_at,i.created_at,i.revoked_at
        FROM installations i JOIN connector_credentials c ON c.installation_id=i.id WHERE i.owner_account_id=?1 ORDER BY i.created_at"#)
        .map_err(ApiError::internal)?;
    let raw = statement
        .query_map([principal.account_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(ApiError::internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::internal)?;
    let installations = raw
        .into_iter()
        .map(installation_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(InstallationListResponse { installations }))
}

async fn bind_installation(
    State(cloud): State<Cloud>,
    AxumPath(workspace_id): AxumPath<String>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkspaceBindingRequest>,
) -> Result<(StatusCode, Json<WorkspaceBindingSummary>), ApiError> {
    let principal = user(&cloud, &headers, true)?;
    let workspace_uuid =
        Uuid::parse_str(&workspace_id).map_err(|_| ApiError::bad("invalid workspace id"))?;
    let mut db = lock(&cloud)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    require_installation_owner(
        &tx,
        &input.installation_id.to_string(),
        &principal.account_id,
    )?;
    let owns_workspace: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM memberships WHERE workspace_id=?1 AND account_id=?2 AND role='owner')",
        params![workspace_id,principal.account_id], |row| row.get(0),
    ).map_err(ApiError::internal)?;
    if !owns_workspace {
        return Err(ApiError::forbidden("workspace owner role required"));
    }
    let binding_uuid = Uuid::new_v4();
    tx.execute("INSERT INTO workspace_bindings(id,workspace_id,installation_id,local_workspace_id) VALUES(?1,?2,?3,?4)",
        params![binding_uuid.to_string(),workspace_id,input.installation_id.to_string(),input.local_workspace_id.to_string()]).map_err(map_conflict)?;
    audit(
        &tx,
        "account",
        Some(&principal.account_id),
        "installation.bound",
        Some(&input.installation_id.to_string()),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(WorkspaceBindingSummary {
            id: binding_uuid,
            workspace_id: workspace_uuid,
            installation_id: input.installation_id,
            local_workspace_id: input.local_workspace_id,
            catalog_revision: 0,
        }),
    ))
}

async fn revoke_installation(
    State(cloud): State<Cloud>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<RevokeInstallationResponse>, ApiError> {
    let principal = user(&cloud, &headers, true)?;
    // Registry-before-database is the lock order shared with relay registration.
    // It makes revocation and connector publication one atomic authority decision.
    let mut relays = cloud.relays.lock().await;
    let installation = {
        let mut db = lock(&cloud)?;
        let tx = db.transaction().map_err(ApiError::internal)?;
        require_installation_owner(&tx, &id, &principal.account_id)?;
        let mut installation = installation_by_id(&tx, &id)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE installations SET revoked_at=?1 WHERE id=?2",
            params![now, id],
        )
        .map_err(ApiError::internal)?;
        tx.execute("UPDATE connector_credentials SET revoked_at=?1 WHERE installation_id=?2 AND revoked_at IS NULL", params![now,id]).map_err(ApiError::internal)?;
        audit(
            &tx,
            "account",
            Some(&principal.account_id),
            "installation.revoked",
            Some(&id),
        )?;
        tx.commit().map_err(ApiError::internal)?;
        installation.connection_state = InstallationConnectionState::Revoked;
        installation.revoked_at = Some(parse_time(&now)?);
        installation
    };
    relays.remove(&installation.route_id);
    Ok(Json(RevokeInstallationResponse { installation }))
}

async fn put_catalog(
    State(cloud): State<Cloud>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(input): Json<PutCatalogRequest>,
) -> Result<Json<CatalogAcceptedResponse>, ApiError> {
    let connector = connector(&cloud, &headers)?;
    if connector.installation_id != id {
        return Err(ApiError::forbidden(
            "credential does not belong to installation",
        ));
    }
    input
        .validate()
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let mut seen = HashSet::with_capacity(input.projects.len());
    for project in &input.projects {
        if !seen.insert(project.slug.as_str().to_owned()) {
            return Err(ApiError::bad("catalog contains duplicate project slug"));
        }
    }

    let mut db = lock(&cloud)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let current: i64 = tx.query_row(r#"SELECT b.catalog_revision FROM workspace_bindings b JOIN installations i ON i.id=b.installation_id
        WHERE b.id=?1 AND b.installation_id=?2 AND i.revoked_at IS NULL"#,
        params![input.binding_id.to_string(),id], |row| row.get(0))
        .optional().map_err(ApiError::internal)?.ok_or_else(|| ApiError::not_found("active workspace binding"))?;
    let revision = i64::try_from(input.revision)
        .map_err(|_| ApiError::bad("catalog revision is too large"))?;
    if revision <= current {
        return Err(ApiError::conflict(
            "catalog revision must increase monotonically",
        ));
    }
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "DELETE FROM catalog_projects WHERE binding_id=?1",
        [input.binding_id.to_string()],
    )
    .map_err(ApiError::internal)?;
    for project in &input.projects {
        tx.execute("INSERT INTO catalog_projects(binding_id,local_project_id,slug,name,description,default_branch,head_sha,revision,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![input.binding_id.to_string(),project.local_project_id.to_string(),project.slug.as_str(),project.name,project.description,project.default_branch,project.head_sha,revision,project.updated_at.to_rfc3339()]).map_err(ApiError::internal)?;
    }
    tx.execute(
        "UPDATE workspace_bindings SET catalog_revision=?1 WHERE id=?2",
        params![revision, input.binding_id.to_string()],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        "installation",
        Some(&id),
        "catalog.replaced",
        Some(&id),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    let _ = now;
    Ok(Json(CatalogAcceptedResponse {
        binding_id: input.binding_id,
        revision: input.revision,
        accepted_projects: input.projects.len(),
    }))
}

async fn heartbeat(
    State(cloud): State<Cloud>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<RelayHeartbeatResponse>, ApiError> {
    let connector = connector(&cloud, &headers)?;
    if connector.installation_id != id {
        return Err(ApiError::forbidden(
            "credential does not belong to installation",
        ));
    }
    let installation_id = parse_uuid(&id)?;
    let now = Utc::now();
    let relays = cloud.relays.lock().await;
    let db = lock(&cloud)?;
    let route: Option<String> = db
        .query_row(
            "SELECT route_id FROM installations WHERE id=?1 AND revoked_at IS NULL",
            [&id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let route = route.ok_or_else(|| ApiError::not_found("active installation"))?;
    if !relays.contains_key(&route) {
        return Err(ApiError::conflict("installation has no live relay session"));
    }
    let generation: i64 = db.query_row("SELECT COALESCE(MAX(catalog_revision),0) FROM workspace_bindings WHERE installation_id=?1", [&id], |row| row.get(0)).map_err(ApiError::internal)?;
    Ok(Json(RelayHeartbeatResponse {
        installation_id,
        generation: generation as u64,
        observed_at: now,
    }))
}

async fn directory_project(
    State(cloud): State<Cloud>,
    AxumPath((workspace, project)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<DirectoryProjectResponse>, ApiError> {
    let principal = user(&cloud, &headers, false)?;
    let _workspace_name: WorkspaceNamespace = workspace
        .parse()
        .map_err(|_| ApiError::not_found("directory project"))?;
    let project_slug: c6_cloud_core::ProjectSlug = project
        .parse()
        .map_err(|_| ApiError::not_found("directory project"))?;
    let db = lock(&cloud)?;
    let row: Option<DirectoryRow> = db
        .query_row(
            r#"
        SELECT w.id,w.namespace,w.display_name,w.owner_account_id,w.created_at,
          p.local_project_id,p.name,p.description,p.default_branch,p.head_sha,p.updated_at,
          i.id,i.local_server_id,i.connected_at,i.route_id,m.role
        FROM workspaces w JOIN workspace_bindings b ON b.workspace_id=w.id
        JOIN installations i ON i.id=b.installation_id AND i.revoked_at IS NULL
        JOIN catalog_projects p ON p.binding_id=b.id
        JOIN memberships m ON m.workspace_id=w.id AND m.account_id=?3
        WHERE w.namespace=?1 AND p.slug=?2
    "#,
            params![workspace, project, principal.account_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                ))
            },
        )
        .optional()
        .map_err(ApiError::internal)?;
    let (
        workspace_id,
        namespace,
        workspace_display,
        owner,
        workspace_created,
        local_project_id,
        name,
        description,
        default_branch,
        head_sha,
        updated_at,
        installation_id,
        _local_server_id,
        _connected_at,
        route_id,
        membership_role,
    ) = row.ok_or_else(|| ApiError::not_found("directory project"))?;
    let installation = installation_by_id(&db, &installation_id)?;
    Ok(Json(DirectoryProjectResponse {
        workspace: CloudWorkspaceSummary {
            id: parse_uuid(&workspace_id)?,
            namespace: namespace.parse().map_err(ApiError::internal)?,
            name: workspace_display,
            owner_account_id: parse_uuid(&owner)?,
            role: parse_role(&membership_role)?,
            created_at: parse_time(&workspace_created)?,
        },
        project: c6_cloud_core::CatalogProject {
            binding_id: binding_for_workspace(&db, &workspace_id)?,
            local_project_id: parse_uuid(&local_project_id)?,
            slug: project_slug,
            name,
            description,
            default_branch,
            head_sha,
            updated_at: parse_time(&updated_at)?,
        },
        installation,
        relay_url: format!(
            "{}/relay/{route_id}/projects/{project}",
            cloud.config.public_origin
        ),
    }))
}

async fn relay_connect(
    State(cloud): State<Cloud>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let connector = connector(&cloud, &headers)?;
    let requested = headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !requested
        .split(',')
        .any(|value| value.trim() == RELAY_SUBPROTOCOL)
    {
        return Err(ApiError::bad(
            "WebSocket subprotocol c6-relay-v1 is required",
        ));
    }
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthenticated)?
        .to_owned();
    let installation_id = connector.installation_id;
    Ok(ws
        .protocols([RELAY_SUBPROTOCOL])
        .on_upgrade(move |socket| relay_session(socket, cloud, installation_id, authorization))
        .into_response())
}

async fn relay_proxy(
    State(cloud): State<Cloud>,
    AxumPath((route, path)): AxumPath<(String, String)>,
    request: Request<Body>,
) -> Response {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    let registration = { cloud.relays.lock().await.get(&route).cloned() };
    let Some(registration) = registration else {
        return ApiError::new(
            StatusCode::BAD_GATEWAY,
            "relay_unavailable",
            "installation is not connected",
        )
        .into_response();
    };
    let (parts, body) = request.into_parts();
    let target = match parts.uri.query() {
        Some(query) => format!("/{path}?{query}"),
        None => format!("/{path}"),
    };
    let method = parts.method.as_str().to_owned();
    if HttpMethod::new(method.clone()).is_err() {
        return ApiError::bad("unsupported relay method").into_response();
    }
    let mut headers = Vec::new();
    for (name, value) in &parts.headers {
        // This path-based relay exists only for loopback dogfood. It shares an
        // origin with the Cloud account UI, so forwarding its Cookie header
        // would disclose the Cloud session to the local installation. A
        // production relay uses an isolated per-installation origin instead.
        if name == header::COOKIE {
            continue;
        }
        let Ok(value) = value.to_str() else {
            return ApiError::bad("relay headers must be visible ASCII").into_response();
        };
        let field = HeaderField {
            name: name.as_str().to_owned(),
            value: value.to_owned(),
        };
        if field.validate().is_ok() {
            headers.push(field);
        }
    }
    let body =
        match tokio::time::timeout_at(deadline, to_bytes(body, MAX_REQUEST_BODY_BYTES as usize))
            .await
        {
            Ok(Ok(body)) => body.to_vec(),
            Ok(Err(_)) => {
                return ApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "payload_too_large",
                    "relay request body is too large",
                )
                .into_response();
            }
            Err(_) => {
                return ApiError::new(
                    StatusCode::GATEWAY_TIMEOUT,
                    "relay_timeout",
                    "relay request body did not arrive before the deadline",
                )
                .into_response();
            }
        };
    let (tx, rx) = oneshot::channel();
    let admitted = tokio::time::timeout_at(
        deadline,
        registration.sender.send(ProxyExchange {
            method,
            target,
            headers,
            body,
            deadline,
            reply: tx,
        }),
    )
    .await;
    match admitted {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            return ApiError::new(
                StatusCode::BAD_GATEWAY,
                "relay_unavailable",
                "installation disconnected",
            )
            .into_response();
        }
        Err(_) => {
            return ApiError::new(
                StatusCode::GATEWAY_TIMEOUT,
                "relay_timeout",
                "relay queue did not admit the request before the deadline",
            )
            .into_response();
        }
    }
    match tokio::time::timeout_at(deadline, rx).await {
        Ok(Ok(Ok(result))) => proxy_response(result),
        Ok(Ok(Err(message))) => {
            ApiError::new(StatusCode::BAD_GATEWAY, "upstream_failed", message).into_response()
        }
        _ => ApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "relay_timeout",
            "installation did not respond before the deadline",
        )
        .into_response(),
    }
}

fn proxy_response(result: ProxyResult) -> Response {
    let status = StatusCode::from_u16(result.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);
    if let Some(headers) = response.headers_mut() {
        for field in result.headers {
            // Do not let the same-origin dogfood upstream overwrite Cloud's
            // host-only account cookies. Isolated production relay origins may
            // preserve local C6 Set-Cookie after a separate origin audit.
            if field.name.eq_ignore_ascii_case("set-cookie") {
                continue;
            }
            if let (Ok(name), Ok(value)) = (
                header::HeaderName::try_from(field.name),
                HeaderValue::from_str(&field.value),
            ) {
                headers.append(name, value);
            }
        }
    }
    response
        .body(Body::from(result.body))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

async fn relay_session(
    mut socket: WebSocket,
    cloud: Cloud,
    installation_id: String,
    authorization: String,
) {
    let Some(Ok(Message::Text(text))) = socket.recv().await else {
        return;
    };
    let Ok(hello) = serde_json::from_str::<RelayControlFrame>(&text) else {
        return;
    };
    let RelayControlFrame::ClientHello(ref hello_frame) = hello else {
        return;
    };
    if !constant_eq(hello_frame.connector_token.expose_secret(), &authorization) {
        return;
    }
    let mut protocol = RelaySessionState::default();
    if protocol.observe_control(&hello).is_err() {
        return;
    }
    let installation_uuid = match parse_uuid(&installation_id) {
        Ok(id) => id,
        Err(_) => return,
    };
    let ready = RelayControlFrame::ServerReady(ServerReadyFrame {
        installation_id: installation_uuid,
        generation: 1,
        max_concurrent_requests: 1,
        max_chunk_bytes: MAX_BODY_CHUNK_BYTES as u32,
    });
    if send_control(&mut socket, &mut protocol, &ready)
        .await
        .is_err()
    {
        return;
    }
    let session_id = Uuid::new_v4();
    let (sender, mut exchanges) = mpsc::channel(1);
    let route = {
        // Hold the registry lock while rechecking durable revocation state. A
        // revoker uses this same lock order, so an active route cannot be
        // published after its credential/installation has been revoked.
        let mut relays = cloud.relays.lock().await;
        match register_relay_if_active(&cloud, &mut relays, &installation_id, session_id, sender) {
            Ok(Some(route)) => route,
            _ => return,
        }
    };
    loop {
        tokio::select! {
            exchange=exchanges.recv()=>{
                let Some(exchange)=exchange else{break;};
                let result=tokio::time::timeout_at(
                    exchange.deadline,
                    execute_exchange(&mut socket,&mut protocol,exchange.method,exchange.target,exchange.headers,exchange.body),
                ).await;
                let timed_out=result.is_err();
                let result=result.unwrap_or(Err("relay exchange exceeded its total deadline"));
                let failed=result.is_err();
                let _=exchange.reply.send(result);
                // Closing the session is the only safe recovery after dropping
                // an in-progress protocol future: it fences late response frames
                // and lets the connector reconnect with a fresh state machine.
                if timed_out || failed {break;}
                if !protocol.is_ready(){break;}
            }
            incoming=socket.recv()=>{
                match incoming {
                    Some(Ok(Message::Ping(bytes)))=>{if socket.send(Message::Pong(bytes)).await.is_err(){break;}},
                    Some(Ok(Message::Text(text)))=>{
                        let Ok(frame)=serde_json::from_str::<RelayControlFrame>(&text) else{break;};
                        match frame {
                            RelayControlFrame::Ping(ref ping)=>{
                                if protocol.observe_control(&frame).is_err(){break;}
                                let pong=RelayControlFrame::Pong(ping.clone());
                                if send_control(&mut socket,&mut protocol,&pong).await.is_err(){break;}
                            }
                            _=>break,
                        }
                    }
                    _=>break,
                }
            }
        }
    }
    let removed = {
        let mut relays = cloud.relays.lock().await;
        if relays
            .get(&route)
            .is_some_and(|entry| entry.session_id == session_id)
        {
            relays.remove(&route);
            true
        } else {
            false
        }
    };
    if removed && let Ok(db) = lock(&cloud) {
        let _ = db.execute(
            "UPDATE installations SET connected_at=NULL WHERE id=?1 AND revoked_at IS NULL",
            [&installation_id],
        );
    }
}

async fn execute_exchange(
    socket: &mut WebSocket,
    state: &mut RelaySessionState,
    method: String,
    target: String,
    headers: Vec<HeaderField>,
    body: Vec<u8>,
) -> Result<ProxyResult, &'static str> {
    let request_id = Uuid::new_v4();
    let deadline = (Utc::now() + Duration::seconds(30))
        .timestamp_millis()
        .max(1) as u64;
    let start = RelayControlFrame::RequestStart(RequestStartFrame {
        request_id,
        method: HttpMethod::new(method).map_err(|_| "invalid relay method")?,
        target,
        headers,
        deadline_unix_ms: deadline,
    });
    send_control(socket, state, &start).await?;
    for (sequence, chunk) in body.chunks(MAX_BODY_CHUNK_BYTES).enumerate() {
        let frame = RelayBodyFrame {
            kind: RelayBodyKind::RequestChunk,
            request_id,
            sequence: sequence as u32,
            payload: chunk.to_vec(),
        };
        state
            .observe_body(&frame)
            .map_err(|_| "relay protocol rejected request body")?;
        socket
            .send(Message::Binary(
                frame
                    .encode()
                    .map_err(|_| "relay request encoding failed")?
                    .into(),
            ))
            .await
            .map_err(|_| "connector disconnected")?;
    }
    send_control(
        socket,
        state,
        &RelayControlFrame::RequestEnd(RequestIdFrame { request_id }),
    )
    .await?;
    let mut status = None;
    let mut response_headers = Vec::new();
    let mut response_body = Vec::new();
    loop {
        let message = socket
            .recv()
            .await
            .ok_or("connector disconnected")?
            .map_err(|_| "connector transport failed")?;
        match message {
            Message::Text(text) => {
                let frame: RelayControlFrame = serde_json::from_str(&text)
                    .map_err(|_| "connector sent malformed control frame")?;
                state
                    .observe_control(&frame)
                    .map_err(|_| "connector violated relay protocol")?;
                match frame {
                    RelayControlFrame::ResponseStart(frame) if frame.request_id == request_id => {
                        status = Some(frame.status);
                        response_headers = frame.headers;
                    }
                    RelayControlFrame::ResponseEnd(frame) if frame.request_id == request_id => {
                        return Ok(ProxyResult {
                            status: status.ok_or("connector ended response before status")?,
                            headers: response_headers,
                            body: response_body,
                        });
                    }
                    RelayControlFrame::RequestFailed(frame) if frame.request_id == request_id => {
                        return Err(match frame.code {
                            c6_cloud_core::RelayFailureCode::Timeout => "local request timed out",
                            _ => "local C6 request failed",
                        });
                    }
                    RelayControlFrame::Ping(ping) => {
                        send_control(socket, state, &RelayControlFrame::Pong(ping)).await?;
                    }
                    RelayControlFrame::Pong(_) => {}
                    _ => return Err("connector sent a frame for the wrong request"),
                }
            }
            Message::Binary(bytes) => {
                let frame = RelayBodyFrame::decode(&bytes)
                    .map_err(|_| "connector sent malformed response body")?;
                if frame.kind != RelayBodyKind::ResponseChunk || frame.request_id != request_id {
                    return Err("connector sent a body for the wrong request");
                }
                state
                    .observe_body(&frame)
                    .map_err(|_| "connector violated relay body limits")?;
                response_body.extend_from_slice(&frame.payload);
                if response_body.len() as u64 > MAX_RESPONSE_BODY_BYTES {
                    return Err("local response exceeded relay limit");
                }
            }
            Message::Ping(bytes) => {
                socket
                    .send(Message::Pong(bytes))
                    .await
                    .map_err(|_| "connector disconnected")?;
            }
            _ => return Err("connector closed during request"),
        }
    }
}

async fn send_control(
    socket: &mut WebSocket,
    state: &mut RelaySessionState,
    frame: &RelayControlFrame,
) -> Result<(), &'static str> {
    state
        .observe_control(frame)
        .map_err(|_| "relay state rejected control frame")?;
    let text = serde_json::to_string(frame).map_err(|_| "relay control encoding failed")?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| "connector disconnected")
}

fn user(cloud: &Cloud, headers: &HeaderMap, mutation: bool) -> Result<UserPrincipal, ApiError> {
    let token =
        cookie(headers, session_cookie_name(cloud)).ok_or_else(ApiError::unauthenticated)?;
    let token_hash = hash(&token);
    let db = lock(cloud)?;
    let row: Option<(String, String)> = db
        .query_row(
            r#"
        SELECT s.account_id,s.csrf_hash FROM sessions s JOIN accounts a ON a.id=s.account_id
        WHERE s.token_hash=?1 AND s.revoked_at IS NULL AND s.expires_at>?2 AND a.disabled_at IS NULL
    "#,
            params![token_hash, Utc::now().to_rfc3339()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let (account_id, csrf_hash) = row.ok_or_else(ApiError::unauthenticated)?;
    let principal = UserPrincipal {
        account_id,
        csrf_hash,
    };
    if mutation {
        require_origin(cloud, headers)?;
        let cookie_value = cookie(headers, csrf_cookie_name(cloud))
            .ok_or_else(|| ApiError::forbidden("CSRF cookie required"))?;
        let header_value = headers
            .get("x-c6-csrf")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::forbidden("CSRF header required"))?;
        if !constant_eq(&hash(&cookie_value), &principal.csrf_hash)
            || !constant_eq(&cookie_value, header_value)
        {
            return Err(ApiError::forbidden("CSRF verification failed"));
        }
    }
    Ok(principal)
}

fn connector(cloud: &Cloud, headers: &HeaderMap) -> Result<ConnectorPrincipal, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthenticated)?;
    let token = SecretToken::parse(token).map_err(|_| ApiError::unauthenticated())?;
    let parsed = token.parsed();
    if parsed.class != TokenClass::Connector {
        return Err(ApiError::unauthenticated());
    }
    let public_id = parsed.public_id;
    let secret_value = parsed.expose_proof();
    let db = lock(cloud)?;
    let row: Option<(String, String)> = db
        .query_row(
            r#"
        SELECT c.installation_id,c.secret_hash FROM connector_credentials c
        JOIN installations i ON i.id=c.installation_id
        WHERE c.public_id=?1 AND c.revoked_at IS NULL AND i.revoked_at IS NULL
    "#,
            [public_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let (installation_id, expected) = row.ok_or_else(ApiError::unauthenticated)?;
    if !constant_eq(&expected, &hash(secret_value)) {
        return Err(ApiError::unauthenticated());
    }
    Ok(ConnectorPrincipal { installation_id })
}

fn require_origin(cloud: &Cloud, headers: &HeaderMap) -> Result<(), ApiError> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::forbidden("Origin header required"))?;
    if !constant_eq(origin, &cloud.config.public_origin) {
        return Err(ApiError::forbidden("Origin does not match this cloud"));
    }
    Ok(())
}

fn require_installation_owner(
    db: &Connection,
    installation_id: &str,
    account_id: &str,
) -> Result<(), ApiError> {
    let owner: Option<String> = db
        .query_row(
            "SELECT owner_account_id FROM installations WHERE id=?1",
            [installation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ApiError::internal)?;
    match owner {
        None => Err(ApiError::not_found("installation")),
        Some(owner) if owner == account_id => Ok(()),
        Some(_) => Err(ApiError::forbidden("installation owner required")),
    }
}

fn register_relay_if_active(
    cloud: &Cloud,
    relays: &mut HashMap<String, RelayRegistration>,
    installation_id: &str,
    session_id: Uuid,
    sender: mpsc::Sender<ProxyExchange>,
) -> Result<Option<String>, ApiError> {
    let db = lock(cloud)?;
    let route: Option<String> = db
        .query_row(
            r#"SELECT i.route_id FROM installations i
               JOIN connector_credentials c ON c.installation_id=i.id
               WHERE i.id=?1 AND i.revoked_at IS NULL AND c.revoked_at IS NULL"#,
            [installation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let Some(route) = route else {
        return Ok(None);
    };
    db.execute(
        "UPDATE installations SET connected_at=?1 WHERE id=?2 AND revoked_at IS NULL",
        params![Utc::now().to_rfc3339(), installation_id],
    )
    .map_err(ApiError::internal)?;
    relays.insert(route.clone(), RelayRegistration { session_id, sender });
    Ok(Some(route))
}

fn create_session(db: &Connection, account_id: &str) -> Result<(String, String, String), ApiError> {
    let token = issue_token(TokenClass::CloudSession)
        .expose_secret()
        .to_owned();
    let csrf = issue_token(TokenClass::Csrf).expose_secret().to_owned();
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    db.execute("INSERT INTO sessions(id,account_id,token_hash,csrf_hash,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![id,account_id,hash(&token),hash(&csrf),now.to_rfc3339(),(now+Duration::hours(SESSION_HOURS)).to_rfc3339()])
        .map_err(ApiError::internal)?;
    Ok((token, csrf, id))
}

fn session_response<T: Serialize>(
    cloud: &Cloud,
    session: &str,
    csrf: &str,
    value: T,
) -> Result<Response, ApiError> {
    let mut response = (StatusCode::CREATED, Json(value)).into_response();
    let suffix = if cloud.secure_cookies { "; Secure" } else { "" };
    let session_cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{suffix}",
        session_cookie_name(cloud),
        session,
        SESSION_HOURS * 3600
    );
    let csrf_cookie = format!(
        "{}={}; Path=/; SameSite=Lax; Max-Age={}{suffix}",
        csrf_cookie_name(cloud),
        csrf,
        SESSION_HOURS * 3600
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie).map_err(ApiError::internal)?,
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&csrf_cookie).map_err(ApiError::internal)?,
    );
    Ok(response)
}

fn append_clear_cookies(cloud: &Cloud, headers: &mut HeaderMap) {
    let suffix = if cloud.secure_cookies { "; Secure" } else { "" };
    for (name, http_only) in [
        (session_cookie_name(cloud), "; HttpOnly"),
        (csrf_cookie_name(cloud), ""),
    ] {
        let value = format!("{name}=; Path=/; SameSite=Lax; Max-Age=0{http_only}{suffix}");
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.append(header::SET_COOKIE, value);
        }
    }
}

fn session_cookie_name(cloud: &Cloud) -> &'static str {
    if cloud.secure_cookies {
        SECURE_SESSION_COOKIE
    } else {
        SESSION_COOKIE
    }
}
fn csrf_cookie_name(cloud: &Cloud) -> &'static str {
    if cloud.secure_cookies {
        SECURE_CSRF_COOKIE
    } else {
        CSRF_COOKIE
    }
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

fn lock(cloud: &Cloud) -> Result<std::sync::MutexGuard<'_, Connection>, ApiError> {
    cloud
        .db
        .lock()
        .map_err(|_| ApiError::internal("cloud database lock poisoned"))
}

fn audit(
    db: &Connection,
    actor_type: &str,
    actor_id: Option<&str>,
    action: &str,
    target_id: Option<&str>,
) -> Result<(), ApiError> {
    db.execute("INSERT INTO audit_events(id,actor_type,actor_id,action,target_id,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![Uuid::new_v4().to_string(),actor_type,actor_id,action,target_id,Utc::now().to_rfc3339()]).map_err(ApiError::internal)?;
    Ok(())
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn constant_eq(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}
fn secret(bytes: usize) -> String {
    let mut data = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut data);
    URL_SAFE_NO_PAD.encode(data)
}
fn short_id() -> String {
    secret(12)
}
fn issue_token(class: TokenClass) -> SecretToken {
    let mut public = [0_u8; 8];
    rand::rng().fill_bytes(&mut public);
    let public = public
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    SecretToken::parse(format!("{}_{}_{}", class.prefix(), public, secret(32)))
        .expect("generated token must satisfy contract")
}
fn parse_uuid(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(ApiError::internal)
}
fn parse_time(value: &str) -> Result<chrono::DateTime<Utc>, ApiError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(ApiError::internal)
}
fn parse_role(value: &str) -> Result<CloudWorkspaceRole, ApiError> {
    match value {
        "owner" => Ok(CloudWorkspaceRole::Owner),
        "maintainer" => Ok(CloudWorkspaceRole::Maintainer),
        "member" => Ok(CloudWorkspaceRole::Member),
        _ => Err(ApiError::internal("invalid stored workspace role")),
    }
}
type InstallationRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
);
type DirectoryRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
);
fn installation_from_row(row: InstallationRow) -> Result<InstallationSummary, ApiError> {
    let (id, local_server_id, route_id, owner, label, public_id, connected, created, revoked) = row;
    Ok(InstallationSummary {
        id: parse_uuid(&id)?,
        local_server_id: parse_uuid(&local_server_id)?,
        route_id,
        owner_account_id: parse_uuid(&owner)?,
        label: label.parse().map_err(ApiError::internal)?,
        credential_public_id: public_id,
        connection_state: if revoked.is_some() {
            InstallationConnectionState::Revoked
        } else if connected.is_some() {
            InstallationConnectionState::Connected
        } else {
            InstallationConnectionState::Disconnected
        },
        connected_at: connected.as_deref().map(parse_time).transpose()?,
        created_at: parse_time(&created)?,
        revoked_at: revoked.as_deref().map(parse_time).transpose()?,
    })
}
fn installation_by_id(db: &Connection, id: &str) -> Result<InstallationSummary, ApiError> {
    let row:Option<InstallationRow>=db.query_row(r#"SELECT i.id,i.local_server_id,i.route_id,i.owner_account_id,i.label,c.public_id,i.connected_at,i.created_at,i.revoked_at
        FROM installations i JOIN connector_credentials c ON c.installation_id=i.id WHERE i.id=?1"#,[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?))).optional().map_err(ApiError::internal)?;
    installation_from_row(row.ok_or_else(|| ApiError::not_found("installation"))?)
}
fn binding_for_workspace(db: &Connection, workspace_id: &str) -> Result<Uuid, ApiError> {
    let id: String = db
        .query_row(
            "SELECT id FROM workspace_bindings WHERE workspace_id=?1",
            [workspace_id],
            |row| row.get(0),
        )
        .map_err(ApiError::internal)?;
    parse_uuid(&id)
}
fn map_conflict(error: rusqlite::Error) -> ApiError {
    if matches!(error, rusqlite::Error::SqliteFailure(ref e, _) if e.extended_code == 2067 || e.extended_code == 1555)
    {
        ApiError::conflict("identifier is already in use")
    } else {
        ApiError::internal(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::Value;
    use tempfile::TempDir;
    use tower::ServiceExt;

    struct Harness {
        _temp: TempDir,
        cloud: Cloud,
        origin: String,
    }

    impl Harness {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let origin = "http://cloud.test".to_owned();
            let cloud = Cloud::open(Config {
                data_dir: temp.path().join("data"),
                public_origin: origin.clone(),
                web_dir: temp.path().join("web"),
            })
            .unwrap();
            Self {
                _temp: temp,
                cloud,
                origin,
            }
        }

        async fn request(
            &self,
            method: &str,
            uri: &str,
            body: Value,
            headers: &[(&str, &str)],
            peer: [u8; 4],
        ) -> Response {
            let mut builder = Request::builder()
                .method(method)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json");
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            let mut request = builder.body(Body::from(body.to_string())).unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo(SocketAddr::from((peer, 4242))));
            app(self.cloud.clone()).oneshot(request).await.unwrap()
        }

        async fn claim(&self) -> (String, String, Value) {
            let token = fs::read_to_string(self.cloud.bootstrap_token_path()).unwrap();
            let response=self.request("POST","/api/v1/bootstrap/claim",json!({
                "bootstrapToken":token.trim(),"handle":"dogfood","displayName":"Dogfood Owner"
            }),&[("origin",&self.origin)],[127,0,0,1]).await;
            assert_eq!(response.status(), StatusCode::CREATED);
            let cookies = response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .map(|value| {
                    value
                        .to_str()
                        .unwrap()
                        .split(';')
                        .next()
                        .unwrap()
                        .to_owned()
                })
                .collect::<Vec<_>>();
            let session = cookies
                .iter()
                .find(|value| value.starts_with("c6_cloud_session="))
                .unwrap()
                .clone();
            let csrf_cookie = cookies
                .iter()
                .find(|value| value.starts_with("c6_cloud_csrf="))
                .unwrap()
                .clone();
            let csrf = csrf_cookie.split_once('=').unwrap().1.to_owned();
            let cookie = format!("{session}; {csrf_cookie}");
            let value = body_json(response).await;
            (cookie, csrf, value)
        }
    }

    async fn body_json(response: Response) -> Value {
        serde_json::from_slice(&to_bytes(response.into_body(), MAX_BODY).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn bootstrap_is_loopback_origin_bound_and_single_use() {
        let harness = Harness::new();
        let token = fs::read_to_string(harness.cloud.bootstrap_token_path()).unwrap();
        let body =
            json!({"bootstrapToken":token.trim(),"handle":"dogfood","displayName":"Dogfood"});
        let remote = harness
            .request(
                "POST",
                "/api/v1/bootstrap/claim",
                body.clone(),
                &[("origin", &harness.origin)],
                [10, 0, 0, 2],
            )
            .await;
        assert_eq!(remote.status(), StatusCode::FORBIDDEN);
        let cross_origin = harness
            .request(
                "POST",
                "/api/v1/bootstrap/claim",
                body.clone(),
                &[("origin", "https://evil.example")],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
        let (_, _, claimed) = harness.claim().await;
        assert_eq!(claimed["account"]["handle"], "dogfood");
        assert!(!harness.cloud.bootstrap_token_path().exists());
        let again = harness
            .request(
                "POST",
                "/api/v1/bootstrap/claim",
                body,
                &[("origin", &harness.origin)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(again.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn session_mutations_require_matching_origin_and_double_submit_csrf() {
        let harness = Harness::new();
        let (cookie, csrf, _) = harness.claim().await;
        let input = json!({"namespace":"paper-street","name":"Paper Street"});
        let missing = harness
            .request(
                "POST",
                "/api/v1/workspaces",
                input.clone(),
                &[("cookie", &cookie), ("origin", &harness.origin)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);
        let evil = harness
            .request(
                "POST",
                "/api/v1/workspaces",
                input.clone(),
                &[
                    ("cookie", &cookie),
                    ("origin", "https://evil.example"),
                    ("x-c6-csrf", &csrf),
                ],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(evil.status(), StatusCode::FORBIDDEN);
        let created = harness
            .request(
                "POST",
                "/api/v1/workspaces",
                input,
                &[
                    ("cookie", &cookie),
                    ("origin", &harness.origin),
                    ("x-c6-csrf", &csrf),
                ],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let get = harness
            .request(
                "GET",
                "/api/v1/workspaces",
                json!(null),
                &[("cookie", &cookie)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(get.status(), StatusCode::OK);
        assert_eq!(
            body_json(get).await["workspaces"].as_array().unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn connector_credential_is_one_time_hash_only_and_revocable() {
        let harness = Harness::new();
        let (cookie, csrf, _) = harness.claim().await;
        let headers = [
            ("cookie", cookie.as_str()),
            ("origin", harness.origin.as_str()),
            ("x-c6-csrf", csrf.as_str()),
        ];
        let response = harness
            .request(
                "POST",
                "/api/v1/installations",
                json!({
                    "localServerId":Uuid::new_v4(),"label":"Neal laptop"
                }),
                &headers,
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let registered = body_json(response).await;
        let installation_id = registered["installation"]["id"].as_str().unwrap();
        let connector_token = registered["connectorToken"].as_str().unwrap();
        assert!(connector_token.starts_with("c6x_v1_"));

        let list = harness
            .request(
                "GET",
                "/api/v1/installations",
                json!(null),
                &[("cookie", &cookie)],
                [127, 0, 0, 1],
            )
            .await;
        let list_text =
            String::from_utf8(to_bytes(list.into_body(), MAX_BODY).await.unwrap().to_vec())
                .unwrap();
        assert!(!list_text.contains(connector_token));
        {
            let db = lock(&harness.cloud).unwrap();
            let persisted: String = db
                .query_row("SELECT secret_hash FROM connector_credentials", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(!persisted.contains(connector_token));
        }

        let auth = format!("Bearer {connector_token}");
        let heartbeat = harness
            .request(
                "POST",
                &format!("/api/v1/installations/{installation_id}/heartbeat"),
                json!(null),
                &[("authorization", &auth)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(heartbeat.status(), StatusCode::CONFLICT);
        let revoke = harness
            .request(
                "DELETE",
                &format!("/api/v1/installations/{installation_id}"),
                json!(null),
                &headers,
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(revoke.status(), StatusCode::OK);
        let rejected = harness
            .request(
                "POST",
                &format!("/api/v1/installations/{installation_id}/heartbeat"),
                json!(null),
                &[("authorization", &auth)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

        let (sender, _receiver) = mpsc::channel(1);
        let mut relays = harness.cloud.relays.lock().await;
        let registration = register_relay_if_active(
            &harness.cloud,
            &mut relays,
            installation_id,
            Uuid::new_v4(),
            sender,
        )
        .unwrap();
        assert!(registration.is_none());
        assert!(relays.is_empty(), "revoked installation was republished");
    }

    #[tokio::test]
    async fn catalog_is_bounded_monotonic_and_member_discoverable() {
        let harness = Harness::new();
        let (cookie, csrf, _) = harness.claim().await;
        let headers = [
            ("cookie", cookie.as_str()),
            ("origin", harness.origin.as_str()),
            ("x-c6-csrf", csrf.as_str()),
        ];
        let workspace_response = harness
            .request(
                "POST",
                "/api/v1/workspaces",
                json!({"namespace":"paper-street","name":"Paper Street"}),
                &headers,
                [127, 0, 0, 1],
            )
            .await;
        let workspace = body_json(workspace_response).await;
        let workspace_id = workspace["id"].as_str().unwrap();
        let local_server_id = Uuid::new_v4();
        let installation_response = harness
            .request(
                "POST",
                "/api/v1/installations",
                json!({"localServerId":local_server_id,"label":"Laptop"}),
                &headers,
                [127, 0, 0, 1],
            )
            .await;
        let installation = body_json(installation_response).await;
        let installation_id = installation["installation"]["id"].as_str().unwrap();
        let token = installation["connectorToken"].as_str().unwrap();
        let binding_response = harness
            .request(
                "POST",
                &format!("/api/v1/workspaces/{workspace_id}/bindings"),
                json!({
                    "installationId":installation_id,"localWorkspaceId":Uuid::new_v4()
                }),
                &headers,
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(binding_response.status(), StatusCode::CREATED);
        let binding = body_json(binding_response).await;
        let binding_id = binding["id"].as_str().unwrap();
        let catalog = json!({"bindingId":binding_id,"revision":1,"projects":[{
            "localProjectId":Uuid::new_v4(),"slug":"weeknote","name":"Weeknote","description":"Tiny team notes",
            "defaultBranch":"main","headSha":"abc123","updatedAt":Utc::now()
        }]});
        let auth = format!("Bearer {token}");
        let accepted = harness
            .request(
                "PUT",
                &format!("/api/v1/installations/{installation_id}/catalog"),
                catalog.clone(),
                &[("authorization", &auth)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(accepted.status(), StatusCode::OK);
        let workspace_list = harness
            .request(
                "GET",
                "/api/v1/workspaces",
                json!(null),
                &[("cookie", &cookie)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(workspace_list.status(), StatusCode::OK);
        let workspace_list = body_json(workspace_list).await;
        assert_eq!(workspace_list["workspaces"][0]["binding"]["id"], binding_id);
        assert_eq!(
            workspace_list["workspaces"][0]["projects"][0]["slug"],
            "weeknote"
        );
        let replay = harness
            .request(
                "PUT",
                &format!("/api/v1/installations/{installation_id}/catalog"),
                catalog,
                &[("authorization", &auth)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(replay.status(), StatusCode::CONFLICT);
        let anonymous = harness
            .request(
                "GET",
                "/api/v1/directory/paper-street/weeknote",
                json!(null),
                &[],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

        let outsider_cookie = {
            let db = lock(&harness.cloud).unwrap();
            let outsider = Uuid::new_v4().to_string();
            db.execute(
                "INSERT INTO accounts(id,handle,display_name,created_at) VALUES(?1,'outsider','Outsider',?2)",
                params![outsider,Utc::now().to_rfc3339()],
            ).unwrap();
            let (session, csrf, _) = create_session(&db, &outsider).unwrap();
            format!("c6_cloud_session={session}; c6_cloud_csrf={csrf}")
        };
        let outsider = harness
            .request(
                "GET",
                "/api/v1/directory/paper-street/weeknote",
                json!(null),
                &[("cookie", &outsider_cookie)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(outsider.status(), StatusCode::NOT_FOUND);

        let directory = harness
            .request(
                "GET",
                "/api/v1/directory/paper-street/weeknote",
                json!(null),
                &[("cookie", &cookie)],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(directory.status(), StatusCode::OK);
        let directory = body_json(directory).await;
        assert_eq!(directory["project"]["name"], "Weeknote");
        assert_eq!(directory["workspace"]["namespace"], "paper-street");
        let relay_url = directory["relayUrl"].as_str().unwrap();
        assert!(relay_url.starts_with("http://cloud.test/relay/"));
        assert!(relay_url.ends_with("/projects/weeknote"));
    }

    #[tokio::test]
    async fn relay_route_is_registry_bound_bounded_and_strips_authority_headers() {
        let harness = Harness::new();
        let missing = harness
            .request(
                "GET",
                "/relay/not-connected/status",
                json!(null),
                &[],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(missing.status(), StatusCode::BAD_GATEWAY);

        let (sender, mut receiver) = mpsc::channel(1);
        harness.cloud.relays.lock().await.insert(
            "safe-route".into(),
            RelayRegistration {
                session_id: Uuid::new_v4(),
                sender,
            },
        );
        let relay = tokio::spawn(async move {
            let exchange = receiver.recv().await.unwrap();
            assert_eq!(exchange.method, "POST");
            assert_eq!(exchange.target, "/api/status?detail=1");
            assert_eq!(exchange.body, br#"{"hello":"relay"}"#);
            assert!(exchange.headers.iter().any(|field| field.name == "x-demo"));
            assert!(!exchange.headers.iter().any(|field| field.name == "host"));
            assert!(!exchange.headers.iter().any(|field| field.name == "cookie"));
            assert!(
                exchange
                    .reply
                    .send(Ok(ProxyResult {
                        status: 201,
                        headers: vec![
                            HeaderField {
                                name: "content-type".into(),
                                value: "text/plain".into(),
                            },
                            HeaderField {
                                name: "set-cookie".into(),
                                value: "c6_cloud_session=attacker; Path=/".into(),
                            },
                        ],
                        body: b"proxied".to_vec(),
                    }))
                    .is_ok()
            );
        });
        let response = harness
            .request(
                "POST",
                "/relay/safe-route/api/status?detail=1",
                json!({"hello":"relay"}),
                &[
                    ("x-demo", "yes"),
                    ("host", "attacker.example"),
                    ("cookie", "c6_cloud_session=cloud-secret"),
                ],
                [127, 0, 0, 1],
            )
            .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        assert_eq!(
            to_bytes(response.into_body(), MAX_BODY)
                .await
                .unwrap()
                .as_ref(),
            b"proxied"
        );
        relay.await.unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn data_directory_is_private_and_symlinks_are_rejected() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("cloud-data");
        let cloud = Cloud::open(Config {
            data_dir: data.clone(),
            public_origin: "http://cloud.test".into(),
            web_dir: temp.path().join("web"),
        })
        .unwrap();
        assert_eq!(
            fs::metadata(&data).unwrap().permissions().mode() & 0o777,
            0o700
        );
        drop(cloud);

        let target = temp.path().join("real-data");
        fs::create_dir(&target).unwrap();
        let linked = temp.path().join("linked-data");
        symlink(&target, &linked).unwrap();
        assert!(
            Cloud::open(Config {
                data_dir: linked,
                public_origin: "http://cloud.test".into(),
                web_dir: temp.path().join("web"),
            })
            .is_err()
        );

        let shared = temp.path().join("shared-data");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            Cloud::open(Config {
                data_dir: shared.clone(),
                public_origin: "http://cloud.test".into(),
                web_dir: temp.path().join("web"),
            })
            .is_err()
        );
        assert_eq!(
            fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(prepare_data_dir(Path::new("/")).is_err());
    }

    #[tokio::test]
    async fn restart_clears_stale_connector_presence() {
        let harness = Harness::new();
        let (cookie, csrf, _) = harness.claim().await;
        let headers = [
            ("cookie", cookie.as_str()),
            ("origin", harness.origin.as_str()),
            ("x-c6-csrf", csrf.as_str()),
        ];
        let response = harness
            .request(
                "POST",
                "/api/v1/installations",
                json!({"localServerId":Uuid::new_v4(),"label":"Restart test"}),
                &headers,
                [127, 0, 0, 1],
            )
            .await;
        let created = body_json(response).await;
        let installation_id = created["installation"]["id"].as_str().unwrap();
        {
            let db = lock(&harness.cloud).unwrap();
            db.execute(
                "UPDATE installations SET connected_at=?1 WHERE id=?2",
                params![Utc::now().to_rfc3339(), installation_id],
            )
            .unwrap();
        }

        let restarted = Cloud::open(harness.cloud.config.clone()).unwrap();
        let connected: Option<String> = lock(&restarted)
            .unwrap()
            .query_row(
                "SELECT connected_at FROM installations WHERE id=?1",
                [installation_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(connected.is_none());
    }
}

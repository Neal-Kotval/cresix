use std::{
    io::Write,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, bail};
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path as AxumPath, Query, RawQuery, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, delete, get, post, put},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{info, warn};
use uuid::Uuid;

const SESSION_COOKIE: &str = "c6_session";
const CSRF_COOKIE: &str = "c6_csrf";
const SESSION_HOURS: i64 = 24 * 30;

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    git: c6_git::GitStore,
    git_http: Option<c6_git::SmartHttpBackend>,
    git_root: PathBuf,
    bootstrap_token_path: PathBuf,
    public_base_url: String,
    secure_cookies: bool,
}

#[derive(Debug)]
struct ApiError(StatusCode, &'static str, String);

impl ApiError {
    fn bad(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, "bad_request", message.into())
    }
    fn unauthenticated() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "authentication required".into(),
        )
    }
    fn forbidden(message: impl Into<String>) -> Self {
        Self(StatusCode::FORBIDDEN, "forbidden", message.into())
    }
    fn not_found(kind: &'static str) -> Self {
        Self(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{kind} not found"),
        )
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self(StatusCode::CONFLICT, "conflict", message.into())
    }
    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal server error".into(),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(json!({"error": {"code": self.1, "message": self.2}})),
        )
            .into_response()
    }
}

#[derive(Debug, Clone)]
struct Principal {
    user_id: String,
    display_name: String,
    session_id: String,
    csrf_hash: String,
    device_id: Option<String>,
}

#[derive(Debug, Clone)]
struct TokenPrincipal {
    user_id: String,
    display_name: String,
    workspace_id: Option<String>,
    project_id: Option<String>,
}

type InviteRow = (String, String, Option<String>, String, Option<String>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapRequest {
    token: String,
    display_name: String,
    device_label: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InviteRequest {
    role: String,
    expires_in_minutes: Option<i64>,
    workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RedeemRequest {
    token: String,
    display_name: String,
    device_label: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceRequest {
    slug: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRequest {
    workspace_id: String,
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "main_branch")]
    default_branch: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestInput {
    title: String,
    #[serde(default)]
    body: String,
    source_branch: String,
    #[serde(default = "main_branch")]
    target_branch: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentInput {
    revision_sha: String,
    #[serde(default = "production")]
    environment: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunInput {
    job: String,
    kind: String,
    revision_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScheduleInput {
    job: String,
    cron: String,
    timezone: String,
    #[serde(default = "forbid")]
    concurrency: String,
    #[serde(default = "yes")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct SecretInput {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateManifestRequest {
    source: String,
}

#[derive(Debug, Deserialize)]
struct GitRevisionQuery {
    #[serde(default = "main_branch")]
    revision: String,
    limit: Option<usize>,
    recursive: Option<bool>,
}

fn main_branch() -> String {
    "main".into()
}
fn production() -> String {
    "production".into()
}
fn forbid() -> String {
    "forbid".into()
}
fn yes() -> bool {
    true
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "c6_server=info,tower_http=info".into()),
        )
        .init();
    let data_dir = PathBuf::from(std::env::var("C6_DATA_DIR").unwrap_or_else(|_| ".c6".into()));
    let public_base_url = validate_public_base_url(
        &std::env::var("C6_PUBLIC_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into()),
    )?;
    let bind: IpAddr = std::env::var("C6_BIND")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()
        .context("C6_BIND must be an IP address")?;
    validate_exposure(
        bind,
        &public_base_url,
        std::env::var("C6_INSECURE_HTTP").as_deref() == Ok("1"),
    )?;
    let state = open_state(&data_dir, public_base_url)?;
    let port = std::env::var("C6_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8787);
    let address = SocketAddr::new(bind, port);
    let listener = TcpListener::bind(address).await.context("bind C6 server")?;
    info!(%address, data_dir=%data_dir.display(), "C6 is ready");
    axum::serve(listener, app(state)).await.context("serve C6")
}

fn open_state(data_dir: &Path, public_base_url: String) -> anyhow::Result<AppState> {
    let git_http_enabled =
        parse_git_http_enabled(std::env::var("C6_GIT_HTTP_ENABLED").ok().as_deref())?;
    open_state_with_git_http(data_dir, public_base_url, git_http_enabled)
}

fn open_state_with_git_http(
    data_dir: &Path,
    public_base_url: String,
    git_http_enabled: bool,
) -> anyhow::Result<AppState> {
    prepare_data_dir(data_dir)?;
    let mut conn = Connection::open(data_dir.join("c6.sqlite3")).context("open C6 database")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    ensure_server(&mut conn, data_dir)?;
    backfill_server_owner(&mut conn)?;
    let secure_cookies = public_base_url.starts_with("https://");
    let git_root = data_dir.join("git");
    let git = c6_git::GitStore::new(&git_root).context("open C6 Git store")?;
    let git_http = if git_http_enabled {
        Some(
            c6_git::SmartHttpBackend::from_environment(c6_git::SmartHttpLimits::default())
                .context("C6_GIT_HTTP_ENABLED is set but the Git HTTP backend is unavailable")?,
        )
    } else {
        None
    };
    let git_root = git_root
        .canonicalize()
        .context("canonicalize C6 Git store")?;
    Ok(AppState {
        db: Arc::new(Mutex::new(conn)),
        git,
        git_http,
        git_root,
        bootstrap_token_path: data_dir.join("bootstrap-token"),
        public_base_url,
        secure_cookies,
    })
}

fn parse_git_http_enabled(value: Option<&str>) -> anyhow::Result<bool> {
    match value {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        Some(_) => bail!("C6_GIT_HTTP_ENABLED must be exactly true, false, 1, or 0"),
    }
}

fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    conn.execute_batch(r#"
    BEGIN IMMEDIATE;
    CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY, display_name TEXT NOT NULL, revoked_at TEXT);
    CREATE TABLE IF NOT EXISTS devices(id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), label TEXT NOT NULL, public_key TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL, revoked_at TEXT);
    CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), device_id TEXT REFERENCES devices(id), token_hash TEXT NOT NULL UNIQUE, csrf_hash TEXT NOT NULL, created_at TEXT NOT NULL, expires_at TEXT NOT NULL, revoked_at TEXT);
    CREATE TABLE IF NOT EXISTS invites(id TEXT PRIMARY KEY, token_hash TEXT NOT NULL UNIQUE, role TEXT NOT NULL, workspace_id TEXT, created_by TEXT NOT NULL REFERENCES users(id), created_at TEXT NOT NULL, expires_at TEXT NOT NULL, redeemed_at TEXT, redeemed_by TEXT);
    CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL, created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS memberships(workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE, user_id TEXT NOT NULL REFERENCES users(id), role TEXT NOT NULL, PRIMARY KEY(workspace_id,user_id));
    CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE, slug TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL, default_branch TEXT NOT NULL, head_sha TEXT NOT NULL DEFAULT '', published_sha TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(workspace_id,slug));
    CREATE TABLE IF NOT EXISTS pull_requests(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, number INTEGER NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL, source_branch TEXT NOT NULL, target_branch TEXT NOT NULL, author_id TEXT NOT NULL REFERENCES users(id), status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(project_id,number));
    CREATE TABLE IF NOT EXISTS deployments(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, revision_sha TEXT NOT NULL, environment TEXT NOT NULL, status TEXT NOT NULL, created_by TEXT NOT NULL REFERENCES users(id), created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, job TEXT NOT NULL, kind TEXT NOT NULL, revision_sha TEXT NOT NULL, status TEXT NOT NULL, trigger_user TEXT NOT NULL REFERENCES users(id), created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS schedules(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, job TEXT NOT NULL, cron TEXT NOT NULL, timezone TEXT NOT NULL, concurrency TEXT NOT NULL, enabled INTEGER NOT NULL, created_by TEXT NOT NULL REFERENCES users(id), created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS secret_metadata(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, name TEXT NOT NULL, created_by TEXT NOT NULL REFERENCES users(id), created_at TEXT NOT NULL, UNIQUE(project_id,name));
    CREATE TABLE IF NOT EXISTS audit_events(id TEXT PRIMARY KEY, actor_id TEXT, action TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT, details TEXT NOT NULL, created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS credentials(
        id TEXT PRIMARY KEY,
        public_id TEXT NOT NULL UNIQUE,
        verifier TEXT NOT NULL,
        user_id TEXT NOT NULL REFERENCES users(id),
        device_id TEXT REFERENCES devices(id),
        label TEXT NOT NULL,
        credential_type TEXT NOT NULL CHECK(credential_type IN ('cli','git')),
        scopes TEXT NOT NULL,
        workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
        project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
        created_at TEXT NOT NULL,
        expires_at TEXT NOT NULL,
        last_used_at TEXT,
        revoked_at TEXT,
        CHECK(length(label) BETWEEN 1 AND 80),
        CHECK(project_id IS NULL OR workspace_id IS NOT NULL)
    );
    CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);
    CREATE INDEX IF NOT EXISTS idx_credentials_public_id ON credentials(public_id);
    CREATE INDEX IF NOT EXISTS idx_credentials_user ON credentials(user_id,created_at DESC);
    CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_events(created_at DESC);
    COMMIT;
    "#)?;
    Ok(())
}

fn ensure_server(conn: &mut Connection, data_dir: &Path) -> anyhow::Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM settings WHERE key='server_id')",
        [],
        |r| r.get(0),
    )?;
    if exists {
        return Ok(());
    }
    let supplied = std::env::var("C6_BOOTSTRAP_TOKEN").ok();
    let generated_path = supplied.is_none().then(|| data_dir.join("bootstrap-token"));
    let token = match supplied.as_deref() {
        Some(token) => {
            validate_bootstrap_token(token)?;
            token.to_owned()
        }
        None => load_or_create_bootstrap_token(generated_path.as_ref().expect("generated path"))?,
    };
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO settings(key,value) VALUES('server_id',?1)",
        [Uuid::new_v4().to_string()],
    )?;
    tx.execute(
        "INSERT INTO settings(key,value) VALUES('bootstrap_hash',?1)",
        [hash(&token)],
    )?;
    tx.commit()?;
    if supplied.is_none() {
        let path = data_dir.join("bootstrap-token");
        warn!(path=%path.display(), "claim this new C6 server using the one-time token file; the file is deleted after claim");
    } else {
        warn!(
            "claim this new C6 server using C6_BOOTSTRAP_TOKEN; the value is never logged or persisted in plaintext"
        );
    }
    Ok(())
}

fn prepare_data_dir(data_dir: &Path) -> anyhow::Result<()> {
    if let Ok(metadata) = std::fs::symlink_metadata(data_dir) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!("C6_DATA_DIR must be a real directory, not a symlink or file");
        }
    } else {
        std::fs::create_dir_all(data_dir).context("create C6 data directory")?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))
            .context("secure C6_DATA_DIR permissions")?;
    }
    Ok(())
}

/// Older development databases predate the explicit server-owner setting.
/// Pin them once to their earliest active user; normal code never updates it.
fn backfill_server_owner(conn: &mut Connection) -> anyhow::Result<()> {
    let owner: Option<String> = conn
        .query_row(
            "SELECT id FROM users WHERE revoked_at IS NULL ORDER BY rowid LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(owner) = owner {
        conn.execute(
            "INSERT OR IGNORE INTO settings(key,value) VALUES('server_owner_id',?1)",
            [owner],
        )?;
    }
    Ok(())
}

fn app(state: AppState) -> Router {
    let browser_api = Router::new()
        .route("/healthz", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/bootstrap/claim", post(claim))
        .route("/api/v1/invites/redeem", post(redeem_invite))
        .route(
            "/api/v1/manifest/validate",
            post(validate_manifest).layer(DefaultBodyLimit::max(1024 * 1024)),
        )
        .route("/api/v1/session", get(session).delete(logout))
        .route("/api/v1/invites", get(list_invites).post(create_invite))
        .route("/api/v1/peers", get(list_peers))
        .route("/api/v1/peers/{id}", delete(revoke_peer))
        .route("/api/v1/devices", get(list_devices))
        .route("/api/v1/devices/{id}", delete(revoke_device))
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions/{id}", delete(revoke_session))
        .route(
            "/api/v1/credentials",
            get(list_credentials).post(create_credential),
        )
        .route("/api/v1/credentials/{id}", delete(revoke_credential))
        .route("/api/v1/cli/whoami", get(cli_whoami))
        .route(
            "/api/v1/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/api/v1/workspaces/{id}",
            put(update_workspace).delete(delete_workspace),
        )
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route(
            "/api/v1/projects/{id}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/api/v1/projects/{id}/remote", get(project_remote))
        .route(
            "/api/v1/projects/{id}/pull-requests",
            get(list_prs).post(create_pr),
        )
        .route(
            "/api/v1/projects/{id}/deployments",
            get(list_deployments).post(create_deployment),
        )
        .route(
            "/api/v1/projects/{id}/runs",
            get(list_runs).post(create_run),
        )
        .route(
            "/api/v1/projects/{id}/schedules",
            get(list_schedules).post(create_schedule),
        )
        .route(
            "/api/v1/projects/{id}/secrets",
            get(list_secrets).post(create_secret_metadata),
        )
        .route(
            "/api/v1/projects/{id}/secrets/{name}/value",
            put(secret_value_unavailable),
        )
        .route(
            "/api/v1/projects/{id}/repository/branches",
            get(git_branches),
        )
        .route("/api/v1/projects/{id}/repository/commits", get(git_commits))
        .route("/api/v1/projects/{id}/repository/tree", get(git_tree))
        .route(
            "/api/v1/projects/{id}/repository/files/{*path}",
            get(git_file),
        )
        .route("/api/v1/audit", get(list_audit))
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*path}", any(api_not_found))
        .layer(middleware::from_fn_with_state(state.clone(), same_origin))
        .layer(DefaultBodyLimit::max(64 * 1024));
    let git_api = Router::new()
        .route("/{workspace}/{repository}/info/refs", get(git_info_refs))
        .route(
            "/{workspace}/{repository}/git-upload-pack",
            post(git_upload_pack),
        )
        .fallback(git_not_found)
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024));
    let web_dist =
        PathBuf::from(std::env::var("C6_WEB_DIST").unwrap_or_else(|_| "web/dist".into()));
    Router::new()
        .merge(browser_api)
        .nest("/git", git_api)
        .fallback_service(
            ServeDir::new(&web_dist).fallback(ServeFile::new(web_dist.join("index.html"))),
        )
        .with_state(state)
        .layer(middleware::from_fn(security_headers))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                // URI, query, and headers are deliberately excluded: all are
                // attacker-controlled and may contain accidentally supplied secrets.
                tracing::info_span!("http_request", method = %request.method())
            }),
        )
}

#[derive(Debug)]
struct GitHttpError(StatusCode, &'static str);

impl GitHttpError {
    fn unauthenticated() -> Self {
        Self(StatusCode::UNAUTHORIZED, "authentication required")
    }
}

impl IntoResponse for GitHttpError {
    fn into_response(self) -> Response {
        let mut response = (self.0, self.1).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if self.0 == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Basic realm=\"C6 Git\", charset=\"UTF-8\""),
            );
        }
        response
    }
}

async fn git_not_found() -> GitHttpError {
    GitHttpError(StatusCode::NOT_FOUND, "Git endpoint not found")
}

async fn git_info_refs(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((workspace, repository_name)): AxumPath<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Response, GitHttpError> {
    let principal = authenticate_git(&state, &headers)?;
    let project = git_project_slug(&repository_name)?;
    let (project_id, repository) =
        authorize_git_project(&state, &principal, &workspace, project, "reader")?;
    let backend = state.git_http.clone().ok_or(GitHttpError(
        StatusCode::SERVICE_UNAVAILABLE,
        "Git transport unavailable",
    ))?;
    let protocol = git_protocol_header(&headers)?;
    let query = query.ok_or(GitHttpError(
        StatusCode::BAD_REQUEST,
        "missing Git service query",
    ))?;
    let actor = principal.user_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        backend.advertise(&repository, &query, protocol.as_deref(), &actor)
    })
    .await
    .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "Git request failed"))?
    .map_err(map_smart_http_error)?;
    let _ = project_id;
    smart_http_response(result)
}

async fn git_upload_pack(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((workspace, repository_name)): AxumPath<(String, String)>,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> Result<Response, GitHttpError> {
    let principal = authenticate_git(&state, &headers)?;
    let project = git_project_slug(&repository_name)?;
    let (_project_id, repository) =
        authorize_git_project(&state, &principal, &workspace, project, "reader")?;
    let backend = state.git_http.clone().ok_or(GitHttpError(
        StatusCode::SERVICE_UNAVAILABLE,
        "Git transport unavailable",
    ))?;
    let protocol = git_protocol_header(&headers)?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let actor = principal.user_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        backend.upload_pack(
            &repository,
            query.as_deref(),
            content_type.as_deref(),
            protocol.as_deref(),
            &actor,
            &body,
        )
    })
    .await
    .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "Git request failed"))?
    .map_err(map_smart_http_error)?;
    smart_http_response(result)
}

fn authenticate_git(state: &AppState, headers: &HeaderMap) -> Result<TokenPrincipal, GitHttpError> {
    if cookie(headers, SESSION_COOKIE).is_some() {
        return Err(GitHttpError::unauthenticated());
    }
    let values = headers.get_all(header::AUTHORIZATION);
    if values.iter().count() != 1 {
        return Err(GitHttpError::unauthenticated());
    }
    let value = values
        .iter()
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or_else(GitHttpError::unauthenticated)?;
    if value.len() > 1024 {
        return Err(GitHttpError::unauthenticated());
    }
    let encoded = value
        .strip_prefix("Basic ")
        .ok_or_else(GitHttpError::unauthenticated)?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| GitHttpError::unauthenticated())?;
    let decoded = std::str::from_utf8(&decoded).map_err(|_| GitHttpError::unauthenticated())?;
    let (username, token) = decoded
        .split_once(':')
        .ok_or_else(GitHttpError::unauthenticated)?;
    if username != "c6" || token.contains(':') {
        return Err(GitHttpError::unauthenticated());
    }
    authenticate_opaque_token(
        state,
        token,
        c6_core::CredentialType::Git,
        c6_core::CredentialScope::GitRead,
    )
    .map_err(|error| match error.0 {
        StatusCode::FORBIDDEN => GitHttpError(StatusCode::FORBIDDEN, "Git access forbidden"),
        _ => GitHttpError::unauthenticated(),
    })
}

fn git_project_slug(repository_name: &str) -> Result<&str, GitHttpError> {
    repository_name
        .strip_suffix(".git")
        .filter(|slug| !slug.is_empty())
        .ok_or(GitHttpError(StatusCode::NOT_FOUND, "project not found"))
}

fn authorize_git_project(
    state: &AppState,
    principal: &TokenPrincipal,
    workspace_slug: &str,
    project_slug: &str,
    minimum_role: &str,
) -> Result<(String, c6_git::Repository), GitHttpError> {
    validate_slug(workspace_slug)
        .map_err(|_| GitHttpError(StatusCode::NOT_FOUND, "project not found"))?;
    validate_slug(project_slug)
        .map_err(|_| GitHttpError(StatusCode::NOT_FOUND, "project not found"))?;
    let db = state
        .db
        .lock()
        .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "Git request failed"))?;
    let project: Option<(String, String, Option<String>)> = db
        .query_row(
            "SELECT p.id,w.id,m.role FROM projects p JOIN workspaces w ON w.id=p.workspace_id LEFT JOIN memberships m ON m.workspace_id=w.id AND m.user_id=?1 WHERE w.slug=?2 AND p.slug=?3",
            params![principal.user_id, workspace_slug, project_slug],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "Git request failed"))?;
    let (project_id, workspace_id, role) =
        project.ok_or(GitHttpError(StatusCode::NOT_FOUND, "project not found"))?;
    let role = role.ok_or(GitHttpError(StatusCode::NOT_FOUND, "project not found"))?;
    if principal
        .workspace_id
        .as_ref()
        .is_some_and(|restricted| restricted != &workspace_id)
        || principal
            .project_id
            .as_ref()
            .is_some_and(|restricted| restricted != &project_id)
    {
        return Err(GitHttpError(StatusCode::NOT_FOUND, "project not found"));
    }
    if role_rank(&role) < role_rank(minimum_role) {
        return Err(GitHttpError(StatusCode::FORBIDDEN, "Git access forbidden"));
    }
    drop(db);
    let repository = state
        .git
        .open(&project_id)
        .map_err(|_| GitHttpError(StatusCode::NOT_FOUND, "project not found"))?;
    Ok((project_id, repository))
}

fn git_protocol_header(headers: &HeaderMap) -> Result<Option<String>, GitHttpError> {
    if headers.get_all("git-protocol").iter().count() > 1 {
        return Err(GitHttpError(
            StatusCode::BAD_REQUEST,
            "invalid Git-Protocol header",
        ));
    }
    headers
        .get("git-protocol")
        .map(|value| {
            value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| GitHttpError(StatusCode::BAD_REQUEST, "invalid Git-Protocol header"))
        })
        .transpose()
}

fn smart_http_response(result: c6_git::SmartHttpResponse) -> Result<Response, GitHttpError> {
    let status = StatusCode::from_u16(result.status)
        .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "invalid Git response"))?;
    let mut response = (status, result.body).into_response();
    for (name, value) in result.headers {
        let name = HeaderName::try_from(name)
            .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "invalid Git response"))?;
        let value = HeaderValue::try_from(value)
            .map_err(|_| GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "invalid Git response"))?;
        response.headers_mut().insert(name, value);
    }
    Ok(response)
}

fn map_smart_http_error(error: c6_git::SmartHttpError) -> GitHttpError {
    use c6_git::SmartHttpError;
    match error {
        SmartHttpError::InvalidRequest(_) => {
            GitHttpError(StatusCode::BAD_REQUEST, "invalid Git request")
        }
        SmartHttpError::RequestTooLarge => {
            GitHttpError(StatusCode::PAYLOAD_TOO_LARGE, "Git request too large")
        }
        SmartHttpError::Timeout => {
            GitHttpError(StatusCode::GATEWAY_TIMEOUT, "Git request timed out")
        }
        SmartHttpError::Unavailable => {
            GitHttpError(StatusCode::SERVICE_UNAVAILABLE, "Git transport unavailable")
        }
        SmartHttpError::InvalidRepository => {
            GitHttpError(StatusCode::NOT_FOUND, "project not found")
        }
        SmartHttpError::ResponseTooLarge
        | SmartHttpError::BackendFailed
        | SmartHttpError::InvalidResponse => {
            GitHttpError(StatusCode::INTERNAL_SERVER_ERROR, "Git request failed")
        }
    }
}

async fn same_origin(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("Bearer "));
    if bearer {
        validate_optional_origin(&state, request.headers())?;
        return Ok(next.run(request).await);
    }
    if request.method() != Method::GET
        && request.method() != Method::HEAD
        && request.method() != Method::OPTIONS
    {
        let origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::forbidden("Origin header required for unsafe requests"))?;
        if origin.trim_end_matches('/') != state.public_base_url {
            return Err(ApiError::forbidden("cross-origin request rejected"));
        }
        if request
            .headers()
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| !matches!(v, "same-origin" | "same-site" | "none"))
        {
            return Err(ApiError::forbidden("cross-site request rejected"));
        }
    }
    Ok(next.run(request).await)
}

async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let is_api = request.uri().path() == "/api" || request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if is_api {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; object-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'"),
    );
    response
}

async fn api_not_found() -> ApiError {
    ApiError::not_found("API endpoint")
}

fn validate_exposure(
    bind: IpAddr,
    public_base_url: &str,
    insecure_http: bool,
) -> anyhow::Result<()> {
    if !bind.is_loopback() {
        if !insecure_http {
            bail!(
                "refusing C6's plaintext listener on non-loopback C6_BIND; bind loopback behind a reverse proxy or explicitly set C6_INSECURE_HTTP=1 for a trusted private/container hop"
            )
        }
        warn!(%bind, %public_base_url, "C6's direct listener is plaintext on a non-loopback interface; protect this trusted hop from untrusted networks");
    }
    Ok(())
}

fn validate_public_base_url(input: &str) -> anyhow::Result<String> {
    if input.contains('#') {
        bail!("C6_PUBLIC_BASE_URL must not contain a fragment");
    }
    if !(input.starts_with("http://") || input.starts_with("https://")) {
        bail!("C6_PUBLIC_BASE_URL must use a lowercase http:// or https:// scheme");
    }
    let uri: axum::http::Uri = input.parse().context("C6_PUBLIC_BASE_URL is malformed")?;
    let scheme = uri
        .scheme_str()
        .context("C6_PUBLIC_BASE_URL has no scheme")?;
    if !matches!(scheme, "http" | "https") {
        bail!("C6_PUBLIC_BASE_URL must use http or https");
    }
    let authority = uri.authority().context("C6_PUBLIC_BASE_URL has no host")?;
    if authority.as_str().contains('@') {
        bail!("C6_PUBLIC_BASE_URL must not contain credentials");
    }
    if uri.query().is_some() {
        bail!("C6_PUBLIC_BASE_URL must not contain a query");
    }
    if !matches!(uri.path(), "" | "/") {
        bail!("C6_PUBLIC_BASE_URL must be an origin without a path");
    }
    Ok(input.trim_end_matches('/').to_owned())
}

fn write_bootstrap_token(path: &Path, token: &str) -> anyhow::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).context("create bootstrap token file")?;
    file.write_all(token.as_bytes())
        .context("write bootstrap token file")?;
    file.write_all(b"\n")
        .context("finish bootstrap token file")?;
    file.sync_all().context("sync bootstrap token file")
}

fn load_or_create_bootstrap_token(path: &Path) -> anyhow::Result<String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("bootstrap token path must be a regular file");
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o777 != 0o600 {
                    bail!("bootstrap token file must have mode 0600");
                }
            }
            let token = std::fs::read_to_string(path)
                .context("read existing bootstrap token file")?
                .trim_end_matches(['\r', '\n'])
                .to_owned();
            validate_bootstrap_token(&token)?;
            Ok(token)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let token = random_token();
            write_bootstrap_token(path, &token)?;
            Ok(token)
        }
        Err(error) => Err(error).context("inspect bootstrap token file"),
    }
}

fn validate_bootstrap_token(token: &str) -> anyhow::Result<()> {
    if !(32..=256).contains(&token.len())
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("bootstrap token file contains an invalid token");
    }
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state
        .db
        .lock()
        .map_err(ApiError::internal)?
        .query_row("SELECT 1", [], |_| Ok(()))
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}),
    ))
}

async fn status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let db = state.db.lock().map_err(ApiError::internal)?;
    let server_id: String =
        setting(&db, "server_id")?.ok_or_else(|| ApiError::internal("server id missing"))?;
    let claimed: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE revoked_at IS NULL)",
            [],
            |r| r.get(0),
        )
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"serverId":server_id,"serverName":"C6","claimed":claimed,"authentication":"invite_session"}),
    ))
}

async fn claim(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Json(input): Json<BootstrapRequest>,
) -> Result<Response, ApiError> {
    validate_identity(&input.display_name, &input.device_label, &input.public_key)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let claimed: bool = tx
        .query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| r.get(0))
        .map_err(ApiError::internal)?;
    if claimed {
        return Err(ApiError::conflict("server already claimed"));
    }
    let expected = tx
        .query_row(
            "SELECT value FROM settings WHERE key='bootstrap_hash'",
            [],
            |r| r.get::<_, String>(0),
        )
        .map_err(ApiError::internal)?;
    if !secure_eq(&expected, &hash(&input.token)) {
        return Err(ApiError::forbidden("invalid bootstrap token"));
    }
    let user_id = Uuid::new_v4().to_string();
    let device_id = Uuid::new_v4().to_string();
    let now = now();
    tx.execute(
        "INSERT INTO users VALUES(?1,?2,NULL)",
        params![user_id, input.display_name],
    )
    .map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO settings(key,value) VALUES('server_owner_id',?1)",
        [&user_id],
    )
    .map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO devices VALUES(?1,?2,?3,?4,?5,NULL)",
        params![
            device_id,
            user_id,
            input.device_label,
            input.public_key,
            now
        ],
    )
    .map_err(map_conflict)?;
    tx.execute("DELETE FROM settings WHERE key='bootstrap_hash'", [])
        .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&user_id),
        "server.claim",
        "server",
        None,
        json!({}),
    )?;
    let issued = issue_session(&tx, &user_id, &device_id)?;
    tx.commit().map_err(ApiError::internal)?;
    if state.bootstrap_token_path.exists()
        && let Err(error) = std::fs::remove_file(&state.bootstrap_token_path)
    {
        warn!(%error, path=%state.bootstrap_token_path.display(), "claimed server but could not remove bootstrap token file; remove it manually");
    }
    Ok(session_response(
        &state,
        issued,
        json!({"user":{"id":user_id,"displayName":input.display_name}}),
    ))
}

async fn create_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<InviteRequest>,
) -> Result<Json<Value>, ApiError> {
    let principal = authenticate(&state, &headers, true)?;
    require_global_owner(&state, &principal)?;
    validate_role(&input.role)?;
    let minutes = input.expires_in_minutes.unwrap_or(30);
    if !(1..=10_080).contains(&minutes) {
        return Err(ApiError::bad(
            "expiresInMinutes must be between 1 and 10080",
        ));
    }
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    if let Some(ref ws) = input.workspace_id {
        require_role_tx(&tx, &principal.user_id, ws, "owner")?;
    }
    let id = Uuid::new_v4().to_string();
    let token = random_token();
    let created = Utc::now();
    let expires = created + Duration::minutes(minutes);
    tx.execute(
        "INSERT INTO invites VALUES(?1,?2,?3,?4,?5,?6,?7,NULL,NULL)",
        params![
            id,
            hash(&token),
            input.role,
            input.workspace_id,
            principal.user_id,
            created.to_rfc3339(),
            expires.to_rfc3339()
        ],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&principal.user_id),
        "invite.create",
        "invite",
        Some(&id),
        json!({"role":input.role,"workspaceId":input.workspace_id}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"id":id,"token":token,"expiresAt":expires,"inviteUrl":format!("{}/join#token={}",state.public_base_url,token)}),
    ))
}

async fn redeem_invite(
    State(state): State<AppState>,
    Json(input): Json<RedeemRequest>,
) -> Result<Response, ApiError> {
    validate_identity(&input.display_name, &input.device_label, &input.public_key)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let row: Option<InviteRow> = tx
        .query_row(
            "SELECT id,role,workspace_id,expires_at,redeemed_at FROM invites WHERE token_hash=?1",
            [hash(&input.token)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let (invite_id, role, workspace_id, expires_at, redeemed_at) =
        row.ok_or_else(|| ApiError::forbidden("invalid invitation"))?;
    if redeemed_at.is_some() {
        return Err(ApiError::conflict("invitation already redeemed"));
    }
    if parse_time(&expires_at)? <= Utc::now() {
        return Err(ApiError::forbidden("invitation expired"));
    }
    let user_id = Uuid::new_v4().to_string();
    let device_id = Uuid::new_v4().to_string();
    let created = now();
    tx.execute(
        "INSERT INTO users VALUES(?1,?2,NULL)",
        params![user_id, input.display_name],
    )
    .map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO devices VALUES(?1,?2,?3,?4,?5,NULL)",
        params![
            device_id,
            user_id,
            input.device_label,
            input.public_key,
            created
        ],
    )
    .map_err(map_conflict)?;
    if let Some(ws) = workspace_id {
        tx.execute(
            "INSERT INTO memberships VALUES(?1,?2,?3)",
            params![ws, user_id, role],
        )
        .map_err(ApiError::internal)?;
    }
    let changed = tx
        .execute(
            "UPDATE invites SET redeemed_at=?1,redeemed_by=?2 WHERE id=?3 AND redeemed_at IS NULL",
            params![created, user_id, invite_id],
        )
        .map_err(ApiError::internal)?;
    if changed != 1 {
        return Err(ApiError::conflict("invitation already redeemed"));
    }
    audit(
        &tx,
        Some(&user_id),
        "invite.redeem",
        "invite",
        Some(&invite_id),
        json!({}),
    )?;
    let issued = issue_session(&tx, &user_id, &device_id)?;
    tx.commit().map_err(ApiError::internal)?;
    Ok(session_response(
        &state,
        issued,
        json!({"user":{"id":user_id,"displayName":input.display_name}}),
    ))
}

async fn session(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let token = cookie(&headers, SESSION_COOKIE).ok_or_else(ApiError::unauthenticated)?;
    let csrf = cookie(&headers, CSRF_COOKIE).ok_or_else(ApiError::unauthenticated)?;
    let p = authenticate(&state, &headers, false)?;
    if !secure_eq(&p.csrf_hash, &hash(&csrf)) {
        return Err(ApiError::unauthenticated());
    }
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let checked_at = now();
    let expires = (Utc::now() + Duration::hours(SESSION_HOURS)).to_rfc3339();
    let renewed = tx.execute(
        "UPDATE sessions SET expires_at=?1 WHERE id=?2 AND token_hash=?3 AND csrf_hash=?4 AND revoked_at IS NULL AND expires_at>?5 AND EXISTS(SELECT 1 FROM users u WHERE u.id=sessions.user_id AND u.revoked_at IS NULL) AND (device_id IS NULL OR EXISTS(SELECT 1 FROM devices d WHERE d.id=sessions.device_id AND d.revoked_at IS NULL))",
        params![expires, p.session_id, hash(&token), hash(&csrf), checked_at],
    ).map_err(ApiError::internal)?;
    if renewed != 1 {
        return Err(ApiError::unauthenticated());
    }
    let workspaces = query_json(
        &tx,
        "SELECT json_object('id',w.id,'slug',w.slug,'name',w.name,'role',m.role) FROM workspaces w JOIN memberships m ON m.workspace_id=w.id WHERE m.user_id=?1 ORDER BY w.name",
        [&p.user_id],
    )?;
    let server_administrator = setting(&tx, "server_owner_id")?.as_deref() == Some(&p.user_id);
    tx.commit().map_err(ApiError::internal)?;
    let mut response = Json(
        json!({"user":{"id":p.user_id,"displayName":p.display_name},"workspaces":workspaces,"serverAdministrator":server_administrator,"session":{"expiresAt":expires}}),
    ).into_response();
    append_session_cookies(&mut response, &state, &token, &csrf);
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute(
        "UPDATE sessions SET revoked_at=?1 WHERE id=?2",
        params![now(), p.session_id],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "session.logout",
        "session",
        Some(&p.session_id),
        json!({}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    let secure = if state.secure_cookies { "; Secure" } else { "" };
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}"
        ))
        .expect("cookie"),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{CSRF_COOKIE}=; Path=/; SameSite=Strict; Max-Age=0{secure}"
        ))
        .expect("cookie"),
    );
    Ok(response)
}

async fn list_invites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, false)?;
    require_global_owner(&state, &p)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"invites":query_json(&db,"SELECT json_object('id',id,'role',role,'workspaceId',workspace_id,'expiresAt',expires_at,'redeemedAt',redeemed_at) FROM invites ORDER BY created_at DESC", [])?}),
    ))
}
async fn list_peers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, false)?;
    require_global_owner(&state, &p)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"peers":query_json(&db,"SELECT json_object('id',id,'displayName',display_name,'revokedAt',revoked_at) FROM users ORDER BY display_name", [])?}),
    ))
}
async fn revoke_peer(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    require_global_owner(&state, &p)?;
    if p.user_id == id {
        return Err(ApiError::bad("cannot revoke your own peer"));
    }
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    if tx
        .execute(
            "UPDATE users SET revoked_at=?1 WHERE id=?2 AND revoked_at IS NULL",
            params![now(), id],
        )
        .map_err(ApiError::internal)?
        == 0
    {
        return Err(ApiError::not_found("peer"));
    }
    tx.execute(
        "UPDATE sessions SET revoked_at=?1 WHERE user_id=?2 AND revoked_at IS NULL",
        params![now(), id],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "peer.revoke",
        "peer",
        Some(&id),
        json!({}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, false)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"devices":query_json(&db,"SELECT json_object('id',id,'label',label,'createdAt',created_at,'revokedAt',revoked_at) FROM devices WHERE user_id=?1 ORDER BY created_at DESC",[&p.user_id])?}),
    ))
}
async fn revoke_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    if tx
        .execute(
            "UPDATE devices SET revoked_at=?1 WHERE id=?2 AND user_id=?3 AND revoked_at IS NULL",
            params![now(), id, p.user_id],
        )
        .map_err(ApiError::internal)?
        == 0
    {
        return Err(ApiError::not_found("device"));
    }
    tx.execute(
        "UPDATE sessions SET revoked_at=?1 WHERE device_id=?2",
        params![now(), id],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "device.revoke",
        "device",
        Some(&id),
        json!({}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, false)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"sessions":query_json(&db,"SELECT json_object('id',id,'deviceId',device_id,'createdAt',created_at,'expiresAt',expires_at,'revokedAt',revoked_at) FROM sessions WHERE user_id=?1 ORDER BY created_at DESC",[&p.user_id])?}),
    ))
}
async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    if tx
        .execute(
            "UPDATE sessions SET revoked_at=?1 WHERE id=?2 AND user_id=?3 AND revoked_at IS NULL",
            params![now(), id, p.user_id],
        )
        .map_err(ApiError::internal)?
        == 0
    {
        return Err(ApiError::not_found("session"));
    }
    audit(
        &tx,
        Some(&p.user_id),
        "session.revoke",
        "session",
        Some(&id),
        json!({}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<c6_core::CreateCredentialRequest>,
) -> Result<(StatusCode, Json<c6_core::CreateCredentialResponse>), ApiError> {
    let principal = authenticate(&state, &headers, true)?;
    let credential_type = input.credential_type.as_str();
    let prefix = input.credential_type.token_prefix();
    let device_uuid = principal
        .device_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(ApiError::internal)?;
    let user_uuid = Uuid::parse_str(&principal.user_id).map_err(ApiError::internal)?;
    validate_credential_label(&input.label)?;
    let mut scopes = input.scopes.clone();
    scopes.sort();
    scopes.dedup();
    if scopes.is_empty()
        || scopes
            .iter()
            .any(|scope| !input.credential_type.allowed_scopes().contains(scope))
        || (input.credential_type == c6_core::CredentialType::Git
            && scopes.contains(&c6_core::CredentialScope::GitWrite))
    {
        return Err(ApiError::bad("at least one valid scope is required"));
    }
    let created = Utc::now();
    let expires = input.expires_at.unwrap_or(created + Duration::days(30));
    if expires <= created || expires > created + Duration::days(90) {
        return Err(ApiError::bad(
            "expiresAt must be in the future and no more than 90 days away",
        ));
    }
    let (workspace_id, project_id) =
        validate_credential_restriction(&state, &principal.user_id, input.restriction.as_ref())?;

    let id = Uuid::new_v4();
    let id_string = id.to_string();
    let public_id = random_public_id();
    let secret = random_token();
    let token = format!("{prefix}_{public_id}_{secret}");
    let scopes_json = serde_json::to_string(&scopes).map_err(ApiError::internal)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO credentials(id,public_id,verifier,user_id,device_id,label,credential_type,scopes,workspace_id,project_id,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            id_string,
            public_id,
            hash(&token),
            principal.user_id,
            principal.device_id,
            input.label,
            credential_type,
            scopes_json,
            workspace_id,
            project_id,
            created.to_rfc3339(),
            expires.to_rfc3339(),
        ],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&principal.user_id),
        "credential.create",
        "credential",
        Some(&id_string),
        json!({"credentialType":credential_type,"scopes":scopes}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(c6_core::CreateCredentialResponse {
            credential: c6_core::CredentialMetadata {
                id,
                user_id: user_uuid,
                device_id: device_uuid,
                credential_type: input.credential_type,
                label: input.label,
                scopes,
                restriction: input.restriction,
                created_at: created,
                expires_at: expires,
                last_used_at: None,
                revoked_at: None,
            },
            token,
        }),
    ))
}

async fn list_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<c6_core::CredentialListResponse>, ApiError> {
    let principal = authenticate(&state, &headers, false)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    let mut statement = db
        .prepare("SELECT id,device_id,label,credential_type,scopes,workspace_id,project_id,created_at,expires_at,last_used_at,revoked_at FROM credentials WHERE user_id=?1 ORDER BY created_at DESC LIMIT 200")
        .map_err(ApiError::internal)?;
    let rows = statement
        .query_map([&principal.user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })
        .map_err(ApiError::internal)?;
    let credentials = rows
        .map(|result| {
            let (
                id,
                device,
                label,
                kind,
                scopes,
                workspace,
                project,
                created,
                expires,
                used,
                revoked,
            ) = result.map_err(ApiError::internal)?;
            let scopes: Vec<c6_core::CredentialScope> =
                serde_json::from_str(&scopes).map_err(ApiError::internal)?;
            let credential_type = kind
                .parse::<c6_core::CredentialType>()
                .map_err(ApiError::internal)?;
            Ok(c6_core::CredentialMetadata {
                id: Uuid::parse_str(&id).map_err(ApiError::internal)?,
                user_id: Uuid::parse_str(&principal.user_id).map_err(ApiError::internal)?,
                device_id: device
                    .map(|value| Uuid::parse_str(&value).map_err(ApiError::internal))
                    .transpose()?,
                credential_type,
                label,
                scopes,
                restriction: match workspace {
                    Some(workspace_id) => Some(c6_core::CredentialResourceRestriction {
                        workspace_id: Some(
                            Uuid::parse_str(&workspace_id).map_err(ApiError::internal)?,
                        ),
                        project_id: project
                            .map(|value| Uuid::parse_str(&value).map_err(ApiError::internal))
                            .transpose()?,
                    }),
                    None => None,
                },
                created_at: parse_time(&created)?,
                expires_at: parse_time(&expires)?,
                last_used_at: used.as_deref().map(parse_time).transpose()?,
                revoked_at: revoked.as_deref().map(parse_time).transpose()?,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(c6_core::CredentialListResponse { credentials }))
}

async fn revoke_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let principal = authenticate(&state, &headers, true)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let owned: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM credentials WHERE id=?1 AND user_id=?2)",
            params![id, principal.user_id],
            |row| row.get(0),
        )
        .map_err(ApiError::internal)?;
    if !owned {
        return Err(ApiError::not_found("credential"));
    }
    let revoked = now();
    let changed = tx
        .execute(
            "UPDATE credentials SET revoked_at=?1 WHERE id=?2 AND user_id=?3 AND revoked_at IS NULL",
            params![revoked, id, principal.user_id],
        )
        .map_err(ApiError::internal)?;
    if changed != 0 {
        audit(
            &tx,
            Some(&principal.user_id),
            "credential.revoke",
            "credential",
            Some(&id),
            json!({}),
        )?;
    }
    tx.commit().map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn cli_whoami(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authenticate_bearer(&state, &headers, c6_core::CredentialScope::ApiRead)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    let server_id =
        setting(&db, "server_id")?.ok_or_else(|| ApiError::internal("missing server id"))?;
    let workspace_sql = "SELECT json_object('id',w.id,'slug',w.slug,'name',w.name,'role',m.role) FROM workspaces w JOIN memberships m ON m.workspace_id=w.id WHERE m.user_id=?1";
    let workspaces = if let Some(workspace_id) = &principal.workspace_id {
        query_json(
            &db,
            &format!("{workspace_sql} AND w.id=?2 ORDER BY w.name"),
            params![principal.user_id, workspace_id],
        )?
    } else {
        query_json(
            &db,
            &format!("{workspace_sql} ORDER BY w.name"),
            [&principal.user_id],
        )?
    };
    Ok(Json(json!({
        "server":{"id":server_id,"name":"C6"},
        "user":{"id":principal.user_id,"displayName":principal.display_name},
        "workspaces":workspaces,
    })))
}

async fn project_remote(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<c6_core::ProjectRemoteResponse>, ApiError> {
    let token_principal = if headers.contains_key(header::AUTHORIZATION) {
        Some(authenticate_bearer(
            &state,
            &headers,
            c6_core::CredentialScope::ApiRead,
        )?)
    } else {
        None
    };
    let user_id = match &token_principal {
        Some(principal) => principal.user_id.clone(),
        None => authenticate(&state, &headers, false)?.user_id,
    };
    let db = state.db.lock().map_err(ApiError::internal)?;
    let project: Option<(String, String, String)> = db
        .query_row(
            "SELECT w.id,w.slug,p.slug FROM projects p JOIN workspaces w ON w.id=p.workspace_id JOIN memberships m ON m.workspace_id=w.id WHERE p.id=?1 AND m.user_id=?2 AND CASE m.role WHEN 'reader' THEN 1 WHEN 'runner' THEN 2 WHEN 'contributor' THEN 3 WHEN 'maintainer' THEN 4 WHEN 'owner' THEN 5 ELSE 0 END >= 1",
            params![id, user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let (workspace_id, workspace_slug, project_slug) =
        project.ok_or_else(|| ApiError::not_found("project"))?;
    if token_principal.as_ref().is_some_and(|principal| {
        principal
            .workspace_id
            .as_ref()
            .is_some_and(|restricted| restricted != &workspace_id)
            || principal
                .project_id
                .as_ref()
                .is_some_and(|restricted| restricted != &id)
    }) {
        return Err(ApiError::not_found("project"));
    }
    Ok(Json(c6_core::ProjectRemoteResponse {
        project_id: Uuid::parse_str(&id).map_err(ApiError::internal)?,
        clone_url: format!(
            "{}/git/{}/{}.git",
            state.public_base_url, workspace_slug, project_slug
        ),
        capabilities: c6_core::RemoteTransportCapabilities {
            fetch: state.git_http.is_some(),
            push: false,
        },
    }))
}

async fn list_workspaces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, false)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"workspaces":query_json(&db,"SELECT json_object('id',w.id,'slug',w.slug,'name',w.name,'role',m.role) FROM workspaces w JOIN memberships m ON m.workspace_id=w.id WHERE m.user_id=?1 ORDER BY w.name",[&p.user_id])?}),
    ))
}
async fn create_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(i): Json<WorkspaceRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = authenticate(&state, &headers, true)?;
    require_global_owner(&state, &p)?;
    validate_slug(&i.slug)?;
    nonempty("name", &i.name, 120)?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO workspaces VALUES(?1,?2,?3,?4)",
        params![id, i.slug, i.name, now()],
    )
    .map_err(map_conflict)?;
    tx.execute(
        "INSERT INTO memberships VALUES(?1,?2,'owner')",
        params![id, p.user_id],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "workspace.create",
        "workspace",
        Some(&id),
        json!({}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":id,"slug":i.slug,"name":i.name,"role":"owner"})),
    ))
}
async fn update_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<WorkspaceRequest>,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    validate_slug(&i.slug)?;
    nonempty("name", &i.name, 120)?;
    require_role(&state, &p, &id, "owner")?;
    let mut db = state.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    if tx
        .execute(
            "UPDATE workspaces SET slug=?1,name=?2 WHERE id=?3",
            params![i.slug, i.name, id],
        )
        .map_err(map_conflict)?
        == 0
    {
        return Err(ApiError::not_found("workspace"));
    }
    audit(
        &tx,
        Some(&p.user_id),
        "workspace.update",
        "workspace",
        Some(&id),
        json!({}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"id":id,"slug":i.slug,"name":i.name,"role":"owner"}),
    ))
}
async fn delete_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    require_role(&state, &p, &id, "owner")?;
    Ok((StatusCode::NOT_IMPLEMENTED, Json(json!({"error":{"code":"not_implemented","message":"workspace deletion is disabled until repository erasure is transactional"}}))).into_response())
}

async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<c6_core::CliProjectListResponse>, ApiError> {
    let token_principal = if headers.contains_key(header::AUTHORIZATION) {
        Some(authenticate_bearer(
            &state,
            &headers,
            c6_core::CredentialScope::ApiRead,
        )?)
    } else {
        None
    };
    let browser_user = if token_principal.is_none() {
        Some(authenticate(&state, &headers, false)?.user_id)
    } else {
        None
    };
    let db = state.db.lock().map_err(ApiError::internal)?;
    let base = "SELECT p.id,p.workspace_id,p.slug,p.name,p.description,p.default_branch,p.head_sha,p.published_sha,m.role,p.updated_at FROM projects p JOIN memberships m ON m.workspace_id=p.workspace_id WHERE m.user_id=?1";
    let projects = if let Some(principal) = token_principal {
        match (&principal.workspace_id, &principal.project_id) {
            (_, Some(project)) => query_cli_projects(
                &db,
                &format!("{base} AND p.id=?2 ORDER BY p.updated_at DESC"),
                params![principal.user_id, project],
            )?,
            (Some(workspace), None) => query_cli_projects(
                &db,
                &format!("{base} AND p.workspace_id=?2 ORDER BY p.updated_at DESC"),
                params![principal.user_id, workspace],
            )?,
            (None, None) => query_cli_projects(
                &db,
                &format!("{base} ORDER BY p.updated_at DESC"),
                [&principal.user_id],
            )?,
        }
    } else {
        query_cli_projects(
            &db,
            &format!("{base} ORDER BY p.updated_at DESC"),
            [browser_user
                .as_deref()
                .expect("browser user was authenticated")],
        )?
    };
    Ok(Json(c6_core::CliProjectListResponse { projects }))
}
async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(i): Json<ProjectRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = authenticate(&state, &headers, true)?;
    require_role(&state, &p, &i.workspace_id, "maintainer")?;
    validate_project(&i)?;
    let caller_role = membership_role(&state, &p.user_id, &i.workspace_id)?;
    // Repository storage is always addressed by a server-generated UUID. User
    // slugs never become filesystem paths.
    let id = Uuid::new_v4().to_string();
    let repository = state.git.create(&id).map_err(map_git)?;
    let readme = format!("# {}\n\n{}\n", i.name, i.description);
    let manifest = "version = 1\n\n[build]\nstrategy = \"auto\"\n";
    let head_sha = match repository.commit_changes(
        &i.default_branch,
        None,
        &[
            c6_git::FileChange::Upsert {
                path: "README.md".into(),
                content: readme.into_bytes(),
            },
            c6_git::FileChange::Upsert {
                path: "c6.toml".into(),
                content: manifest.as_bytes().to_vec(),
            },
        ],
        "Initialize C6 project",
        &c6_git::CommitIdentity {
            name: "C6".into(),
            email: "c6@localhost".into(),
        },
    ) {
        Ok(sha) => sha,
        Err(error) => {
            cleanup_new_repository(&state, &id)?;
            return Err(map_git(error));
        }
    };
    let t = now();
    let database_result = (|| -> Result<(), ApiError> {
        let mut db = state.db.lock().map_err(ApiError::internal)?;
        let tx = db.transaction().map_err(ApiError::internal)?;
        tx.execute("INSERT INTO projects(id,workspace_id,slug,name,description,default_branch,head_sha,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![id,i.workspace_id,i.slug,i.name,i.description,i.default_branch,head_sha,t]).map_err(map_conflict)?;
        audit(
            &tx,
            Some(&p.user_id),
            "project.create",
            "project",
            Some(&id),
            json!({}),
        )?;
        tx.commit().map_err(ApiError::internal)?;
        Ok(())
    })();
    if let Err(error) = database_result {
        cleanup_new_repository(&state, &id)?;
        return Err(error);
    }
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"id":id,"workspaceId":i.workspace_id,"slug":i.slug,"name":i.name,"description":i.description,"defaultBranch":i.default_branch,"headSha":head_sha,"publishedSha":null,"role":caller_role,"updatedAt":t}),
        ),
    ))
}
async fn get_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, false)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    let v:Option<String>=db.query_row("SELECT json_object('id',p.id,'workspaceId',p.workspace_id,'slug',p.slug,'name',p.name,'description',p.description,'defaultBranch',p.default_branch,'headSha',p.head_sha,'publishedSha',p.published_sha,'role',m.role,'updatedAt',p.updated_at) FROM projects p JOIN memberships m ON m.workspace_id=p.workspace_id WHERE p.id=?1 AND m.user_id=?2",params![id,p.user_id],|r|r.get(0)).optional().map_err(ApiError::internal)?;
    Ok(Json(parse_json(
        v.ok_or_else(|| ApiError::not_found("project"))?,
    )?))
}
async fn update_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<ProjectRequest>,
) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    let ws = project_workspace(&state, &id)?;
    require_role(&state, &p, &ws, "maintainer")?;
    validate_project(&i)?;
    if i.workspace_id != ws {
        return Err(ApiError::bad("project cannot move workspaces"));
    }
    let branches = state
        .git
        .open(&id)
        .map_err(map_git)?
        .list_branches()
        .map_err(map_git)?;
    if !branches
        .iter()
        .any(|branch| branch.name == i.default_branch)
    {
        return Err(ApiError::bad("defaultBranch does not exist"));
    }
    {
        let mut db = state.db.lock().map_err(ApiError::internal)?;
        let tx = db.transaction().map_err(ApiError::internal)?;
        tx.execute("UPDATE projects SET slug=?1,name=?2,description=?3,default_branch=?4,updated_at=?5 WHERE id=?6",params![i.slug,i.name,i.description,i.default_branch,now(),id]).map_err(map_conflict)?;
        audit(
            &tx,
            Some(&p.user_id),
            "project.update",
            "project",
            Some(&id),
            json!({}),
        )?;
        tx.commit().map_err(ApiError::internal)?;
    }
    get_project(State(state), headers, AxumPath(id)).await
}
async fn delete_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let p = authenticate(&state, &headers, true)?;
    let ws = project_workspace(&state, &id)?;
    require_role(&state, &p, &ws, "owner")?;
    Ok((StatusCode::NOT_IMPLEMENTED, Json(json!({"error":{"code":"not_implemented","message":"project deletion is disabled until repository erasure is transactional"}}))).into_response())
}

async fn list_prs(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    project_list(
        &s,
        &h,
        &id,
        "reader",
        "pullRequests",
        "SELECT json_object('id',id,'number',number,'title',title,'body',body,'sourceBranch',source_branch,'targetBranch',target_branch,'status',status,'updatedAt',updated_at) FROM pull_requests WHERE project_id=?1 ORDER BY number DESC",
    )
}
async fn create_pr(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<PullRequestInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = project_auth(&s, &h, &id, "contributor", true)?;
    nonempty("title", &i.title, 240)?;
    if i.body.len() > 32 * 1024 {
        return Err(ApiError::bad("pull request body exceeds 32768 bytes"));
    }
    c6_git::validate_branch(&i.source_branch).map_err(map_git)?;
    c6_git::validate_branch(&i.target_branch).map_err(map_git)?;
    let branches = s
        .git
        .open(&id)
        .map_err(map_git)?
        .list_branches()
        .map_err(map_git)?;
    if !branches.iter().any(|branch| branch.name == i.source_branch)
        || !branches.iter().any(|branch| branch.name == i.target_branch)
    {
        return Err(ApiError::bad("pull request branches must exist"));
    }
    let mut db = s.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    let n: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(number),0)+1 FROM pull_requests WHERE project_id=?1",
            [&id],
            |r| r.get(0),
        )
        .map_err(ApiError::internal)?;
    let uid = Uuid::new_v4().to_string();
    let t = now();
    tx.execute(
        "INSERT INTO pull_requests VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'open',?9,?9)",
        params![
            uid,
            id,
            n,
            i.title,
            i.body,
            i.source_branch,
            i.target_branch,
            p.user_id,
            t
        ],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "pull_request.create",
        "pull_request",
        Some(&uid),
        json!({"projectId":id}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"id":uid,"number":n,"title":i.title,"body":i.body,"sourceBranch":i.source_branch,"targetBranch":i.target_branch,"status":"open","updatedAt":t}),
        ),
    ))
}
async fn list_deployments(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    project_list(
        &s,
        &h,
        &id,
        "reader",
        "deployments",
        "SELECT json_object('id',id,'revisionSha',revision_sha,'environment',environment,'status',status,'createdAt',created_at) FROM deployments WHERE project_id=?1 ORDER BY created_at DESC",
    )
}
async fn create_deployment(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<DeploymentInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = project_auth(&s, &h, &id, "maintainer", true)?;
    nonempty("revisionSha", &i.revision_sha, 128)?;
    if !matches!(i.environment.as_str(), "production" | "preview") {
        return Err(ApiError::bad("invalid environment"));
    }
    validate_revision_exists(&s, &id, &i.revision_sha)?;
    let uid = Uuid::new_v4().to_string();
    let t = now();
    let mut db = s.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO deployments VALUES(?1,?2,?3,?4,'recorded',?5,?6)",
        params![uid, id, i.revision_sha, i.environment, p.user_id, t],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "deployment.create",
        "deployment",
        Some(&uid),
        json!({"projectId":id}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"id":uid,"revisionSha":i.revision_sha,"environment":i.environment,"status":"recorded","dispatchAvailable":false,"createdAt":t}),
        ),
    ))
}
async fn list_runs(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    project_list(
        &s,
        &h,
        &id,
        "reader",
        "runs",
        "SELECT json_object('id',id,'job',job,'kind',kind,'revisionSha',revision_sha,'status',status,'createdAt',created_at) FROM runs WHERE project_id=?1 ORDER BY created_at DESC",
    )
}
async fn create_run(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<RunInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = project_auth(&s, &h, &id, "runner", true)?;
    if !matches!(i.kind.as_str(), "command" | "cron" | "agent") {
        return Err(ApiError::bad("invalid run kind"));
    }
    nonempty("job", &i.job, 120)?;
    let rev = match i.revision_sha {
        Some(revision) => revision,
        None => project_head(&s, &id)?,
    };
    validate_revision_exists(&s, &id, &rev)?;
    let uid = Uuid::new_v4().to_string();
    let t = now();
    let mut db = s.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO runs VALUES(?1,?2,?3,?4,?5,'recorded',?6,?7)",
        params![uid, id, i.job, i.kind, rev, p.user_id, t],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "run.record",
        "run",
        Some(&uid),
        json!({"projectId":id,"dispatchAvailable":false}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"id":uid,"job":i.job,"kind":i.kind,"revisionSha":rev,"status":"recorded","dispatchAvailable":false,"createdAt":t}),
        ),
    ))
}
async fn list_schedules(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    project_list(
        &s,
        &h,
        &id,
        "reader",
        "schedules",
        "SELECT json_object('id',id,'job',job,'cron',cron,'timezone',timezone,'concurrency',concurrency,'enabled',json(enabled),'createdAt',created_at) FROM schedules WHERE project_id=?1 ORDER BY created_at DESC",
    )
}
async fn create_schedule(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<ScheduleInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = project_auth(&s, &h, &id, "maintainer", true)?;
    nonempty("job", &i.job, 120)?;
    nonempty("cron", &i.cron, 256)?;
    nonempty("timezone", &i.timezone, 128)?;
    if i.concurrency != "forbid" {
        return Err(ApiError::bad(
            "only forbid concurrency is available in this release",
        ));
    }
    let uid = Uuid::new_v4().to_string();
    let validated = c6_scheduler::ValidatedSchedule::new(c6_scheduler::ScheduleDefinition {
        id: uid.clone(),
        cron: i.cron.clone(),
        timezone: i.timezone.clone(),
        missed_run_policy: c6_scheduler::MissedRunPolicy::RunOnce,
    })
    .map_err(|error| ApiError::bad(error.to_string()))?;
    let next_occurrence = validated
        .next_after(Utc::now())
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let t = now();
    let mut db = s.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO schedules VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            uid,
            id,
            i.job,
            i.cron,
            i.timezone,
            i.concurrency,
            i.enabled,
            p.user_id,
            t
        ],
    )
    .map_err(ApiError::internal)?;
    audit(
        &tx,
        Some(&p.user_id),
        "schedule.create",
        "schedule",
        Some(&uid),
        json!({"projectId":id}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"id":uid,"job":i.job,"cron":i.cron,"timezone":i.timezone,"concurrency":i.concurrency,"enabled":i.enabled,"dispatchAvailable":false,"nextOccurrenceAt":next_occurrence,"createdAt":t}),
        ),
    ))
}
async fn list_secrets(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    project_list(
        &s,
        &h,
        &id,
        "maintainer",
        "secrets",
        "SELECT json_object('id',id,'name',name,'createdAt',created_at) FROM secret_metadata WHERE project_id=?1 ORDER BY name",
    )
}
async fn create_secret_metadata(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(i): Json<SecretInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let p = project_auth(&s, &h, &id, "maintainer", true)?;
    validate_secret_name(&i.name)?;
    let uid = Uuid::new_v4().to_string();
    let t = now();
    let mut db = s.db.lock().map_err(ApiError::internal)?;
    let tx = db.transaction().map_err(ApiError::internal)?;
    tx.execute(
        "INSERT INTO secret_metadata VALUES(?1,?2,?3,?4,?5)",
        params![uid, id, i.name, p.user_id, t],
    )
    .map_err(map_conflict)?;
    audit(
        &tx,
        Some(&p.user_id),
        "secret_metadata.create",
        "secret_metadata",
        Some(&uid),
        json!({"projectId":id}),
    )?;
    tx.commit().map_err(ApiError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":uid,"name":i.name,"createdAt":t})),
    ))
}
async fn secret_value_unavailable(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath((id, _)): AxumPath<(String, String)>,
) -> Result<Response, ApiError> {
    project_auth(&s, &h, &id, "maintainer", true)?;
    Ok((StatusCode::NOT_IMPLEMENTED,Json(json!({"error":{"code":"not_implemented","message":"secret value storage is not available in this release"}}))).into_response())
}
async fn list_audit(State(s): State<AppState>, h: HeaderMap) -> Result<Json<Value>, ApiError> {
    let p = authenticate(&s, &h, false)?;
    require_global_owner(&s, &p)?;
    let db = s.db.lock().map_err(ApiError::internal)?;
    Ok(Json(
        json!({"events":query_json(&db,"SELECT json_object('id',id,'actorId',actor_id,'action',action,'targetType',target_type,'targetId',target_id,'details',json(details),'createdAt',created_at) FROM audit_events ORDER BY created_at DESC LIMIT 500", [])?}),
    ))
}

async fn git_branches(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    project_auth(&s, &h, &id, "reader", false)?;
    let branches = s
        .git
        .open(&id)
        .map_err(map_git)?
        .list_branches()
        .map_err(map_git)?;
    Ok(Json(json!({"branches": branches})))
}

async fn git_commits(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Query(q): Query<GitRevisionQuery>,
) -> Result<Json<Value>, ApiError> {
    project_auth(&s, &h, &id, "reader", false)?;
    let commits = s
        .git
        .open(&id)
        .map_err(map_git)?
        .list_commits(&q.revision, q.limit.unwrap_or(50))
        .map_err(map_git)?;
    Ok(Json(json!({"revision": q.revision, "commits": commits})))
}

async fn git_tree(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Query(q): Query<GitRevisionQuery>,
) -> Result<Json<Value>, ApiError> {
    project_auth(&s, &h, &id, "reader", false)?;
    let entries = s
        .git
        .open(&id)
        .map_err(map_git)?
        .list_tree(&q.revision, q.recursive.unwrap_or(false))
        .map_err(map_git)?;
    Ok(Json(json!({"revision": q.revision, "entries": entries})))
}

async fn git_file(
    State(s): State<AppState>,
    h: HeaderMap,
    AxumPath((id, path)): AxumPath<(String, String)>,
    Query(q): Query<GitRevisionQuery>,
) -> Result<Response, ApiError> {
    project_auth(&s, &h, &id, "reader", false)?;
    let bytes = s
        .git
        .open(&id)
        .map_err(map_git)?
        .read_file(&q.revision, &path)
        .map_err(map_git)?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
}

async fn validate_manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ValidateManifestRequest>,
) -> Result<Json<Value>, ApiError> {
    authenticate(&state, &headers, false)?;
    Ok(match c6_core::ProjectManifest::parse(&input.source) {
        Ok(m) => Json(json!({"valid":true,"manifest":m,"error":null})),
        Err(e) => Json(json!({"valid":false,"manifest":null,"error":e.to_string()})),
    })
}

fn authenticate(state: &AppState, headers: &HeaderMap, csrf: bool) -> Result<Principal, ApiError> {
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(ApiError::unauthenticated());
    }
    let token = cookie(headers, SESSION_COOKIE).ok_or_else(ApiError::unauthenticated)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    let p:Option<Principal>=db.query_row("SELECT u.id,u.display_name,s.id,s.csrf_hash,s.device_id FROM sessions s JOIN users u ON u.id=s.user_id LEFT JOIN devices d ON d.id=s.device_id WHERE s.token_hash=?1 AND s.revoked_at IS NULL AND u.revoked_at IS NULL AND (d.id IS NULL OR d.revoked_at IS NULL) AND s.expires_at>?2",params![hash(&token),now()],|r|Ok(Principal{user_id:r.get(0)?,display_name:r.get(1)?,session_id:r.get(2)?,csrf_hash:r.get(3)?,device_id:r.get(4)?})).optional().map_err(ApiError::internal)?;
    let p = p.ok_or_else(ApiError::unauthenticated)?;
    if csrf {
        let supplied = headers
            .get("x-c6-csrf")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::forbidden("missing CSRF token"))?;
        let csrf_cookie = cookie(headers, CSRF_COOKIE)
            .ok_or_else(|| ApiError::forbidden("missing CSRF cookie"))?;
        if !secure_eq(supplied, &csrf_cookie) || !secure_eq(&p.csrf_hash, &hash(supplied)) {
            return Err(ApiError::forbidden("invalid CSRF token"));
        }
    }
    Ok(p)
}

fn authenticate_bearer(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: c6_core::CredentialScope,
) -> Result<TokenPrincipal, ApiError> {
    if cookie(headers, SESSION_COOKIE).is_some() {
        return Err(ApiError::unauthenticated());
    }
    validate_optional_origin(state, headers)?;
    let values = headers.get_all(header::AUTHORIZATION);
    if values.iter().count() != 1 {
        return Err(ApiError::unauthenticated());
    }
    let authorization = values
        .iter()
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or_else(ApiError::unauthenticated)?;
    let token = authorization
        .strip_prefix("Bearer ")
        .ok_or_else(ApiError::unauthenticated)?;
    authenticate_opaque_token(state, token, c6_core::CredentialType::Cli, required_scope)
}

fn authenticate_opaque_token(
    state: &AppState,
    token: &str,
    expected_type: c6_core::CredentialType,
    required_scope: c6_core::CredentialScope,
) -> Result<TokenPrincipal, ApiError> {
    let public_id = parse_opaque_token(token, expected_type)?;
    type CredentialRow = (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    );
    let db = state.db.lock().map_err(ApiError::internal)?;
    let row: Option<CredentialRow> = db
        .query_row(
            "SELECT c.id,c.verifier,u.id,u.display_name,c.scopes,c.workspace_id,c.project_id FROM credentials c JOIN users u ON u.id=c.user_id LEFT JOIN devices d ON d.id=c.device_id WHERE c.public_id=?1 AND c.credential_type=?2 AND c.revoked_at IS NULL AND c.expires_at>?3 AND u.revoked_at IS NULL AND (c.device_id IS NULL OR (d.id IS NOT NULL AND d.revoked_at IS NULL))",
            params![public_id, expected_type.as_str(), now()],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        )
        .optional()
        .map_err(ApiError::internal)?;
    let (credential_id, verifier, user_id, display_name, scopes_json, workspace_id, project_id) =
        row.ok_or_else(ApiError::unauthenticated)?;
    if !secure_eq(&verifier, &hash(token)) {
        return Err(ApiError::unauthenticated());
    }
    let scopes: Vec<c6_core::CredentialScope> =
        serde_json::from_str(&scopes_json).map_err(ApiError::internal)?;
    if !scopes.contains(&required_scope) {
        return Err(ApiError::forbidden(
            "credential scope does not allow this operation",
        ));
    }
    db.execute(
        "UPDATE credentials SET last_used_at=?1 WHERE id=?2",
        params![now(), credential_id],
    )
    .map_err(ApiError::internal)?;
    Ok(TokenPrincipal {
        user_id,
        display_name,
        workspace_id,
        project_id,
    })
}

fn parse_opaque_token(
    token: &str,
    expected_type: c6_core::CredentialType,
) -> Result<&str, ApiError> {
    if token.len() > 256 || token.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ApiError::unauthenticated());
    }
    let prefix = format!("{}_", expected_type.token_prefix());
    let rest = token
        .strip_prefix(&prefix)
        .ok_or_else(ApiError::unauthenticated)?;
    if rest.len() != 60 || rest.as_bytes().get(16) != Some(&b'_') {
        return Err(ApiError::unauthenticated());
    }
    let public_id = &rest[..16];
    let secret = &rest[17..];
    if public_id.len() != 16
        || secret.len() != 43
        || URL_SAFE_NO_PAD
            .decode(public_id)
            .map_or(true, |bytes| bytes.len() != 12)
        || URL_SAFE_NO_PAD
            .decode(secret)
            .map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(ApiError::unauthenticated());
    }
    Ok(public_id)
}

fn validate_optional_origin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if let Some(origin) = headers.get(header::ORIGIN) {
        let origin = origin
            .to_str()
            .map_err(|_| ApiError::forbidden("invalid Origin header"))?;
        if origin.trim_end_matches('/') != state.public_base_url {
            return Err(ApiError::forbidden("cross-origin request rejected"));
        }
    }
    Ok(())
}
fn issue_session(
    tx: &Transaction<'_>,
    user: &str,
    device: &str,
) -> Result<(String, String, String), ApiError> {
    let id = Uuid::new_v4().to_string();
    let token = random_token();
    let csrf = random_token();
    let created = Utc::now();
    let expires = created + Duration::hours(SESSION_HOURS);
    tx.execute(
        "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7,NULL)",
        params![
            id,
            user,
            device,
            hash(&token),
            hash(&csrf),
            created.to_rfc3339(),
            expires.to_rfc3339()
        ],
    )
    .map_err(ApiError::internal)?;
    Ok((token, csrf, expires.to_rfc3339()))
}
fn session_response(state: &AppState, issued: (String, String, String), body: Value) -> Response {
    let (token, csrf, expires) = issued;
    let mut response = (
        StatusCode::CREATED,
        Json(json!({"session":{"csrfToken":csrf,"expiresAt":expires},"identity":body})),
    )
        .into_response();
    append_session_cookies(&mut response, state, &token, &csrf);
    response
}

fn append_session_cookies(response: &mut Response, state: &AppState, token: &str, csrf: &str) {
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        SESSION_HOURS * 3600,
        if state.secure_cookies { "; Secure" } else { "" }
    );
    let csrf_cookie = format!(
        "{CSRF_COOKIE}={csrf}; Path=/; SameSite=Strict; Max-Age={}{}",
        SESSION_HOURS * 3600,
        if state.secure_cookies { "; Secure" } else { "" }
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("cookie is valid"),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&csrf_cookie).expect("cookie is valid"),
    );
}
fn require_global_owner(state: &AppState, p: &Principal) -> Result<(), ApiError> {
    let db = state.db.lock().map_err(ApiError::internal)?;
    let server_owner = setting(&db, "server_owner_id")?;
    if server_owner.as_deref() == Some(&p.user_id) {
        Ok(())
    } else {
        Err(ApiError::forbidden("server administrator required"))
    }
}
fn require_role(state: &AppState, p: &Principal, ws: &str, min: &str) -> Result<(), ApiError> {
    let db = state.db.lock().map_err(ApiError::internal)?;
    require_role_tx(&db, &p.user_id, ws, min)
}
fn require_role_tx(db: &Connection, user: &str, ws: &str, min: &str) -> Result<(), ApiError> {
    let role: Option<String> = db
        .query_row(
            "SELECT role FROM memberships WHERE workspace_id=?1 AND user_id=?2",
            params![ws, user],
            |r| r.get(0),
        )
        .optional()
        .map_err(ApiError::internal)?;
    if role.is_some_and(|r| role_rank(&r) >= role_rank(min)) {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!("{min} role required")))
    }
}
fn project_auth(
    state: &AppState,
    h: &HeaderMap,
    id: &str,
    min: &str,
    csrf: bool,
) -> Result<Principal, ApiError> {
    let p = authenticate(state, h, csrf)?;
    let ws = project_workspace(state, id)?;
    require_role(state, &p, &ws, min)?;
    Ok(p)
}
fn project_workspace(state: &AppState, id: &str) -> Result<String, ApiError> {
    state
        .db
        .lock()
        .map_err(ApiError::internal)?
        .query_row("SELECT workspace_id FROM projects WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("project"))
}
fn membership_role(state: &AppState, user: &str, workspace: &str) -> Result<String, ApiError> {
    state
        .db
        .lock()
        .map_err(ApiError::internal)?
        .query_row(
            "SELECT role FROM memberships WHERE user_id=?1 AND workspace_id=?2",
            params![user, workspace],
            |row| row.get(0),
        )
        .map_err(ApiError::internal)
}
fn project_head(state: &AppState, id: &str) -> Result<String, ApiError> {
    state
        .db
        .lock()
        .map_err(ApiError::internal)?
        .query_row("SELECT head_sha FROM projects WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .map_err(ApiError::internal)
}
fn validate_revision_exists(state: &AppState, id: &str, revision: &str) -> Result<(), ApiError> {
    if !matches!(revision.len(), 40 | 64)
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ApiError::bad(
            "revisionSha must be a full lowercase Git object ID",
        ));
    }
    state
        .git
        .open(id)
        .map_err(map_git)?
        .list_commits(revision, 1)
        .map_err(map_git)?;
    Ok(())
}
fn project_list(
    state: &AppState,
    h: &HeaderMap,
    id: &str,
    min: &str,
    key: &str,
    sql: &str,
) -> Result<Json<Value>, ApiError> {
    project_auth(state, h, id, min, false)?;
    let db = state.db.lock().map_err(ApiError::internal)?;
    let bounded = format!("{sql} LIMIT 200");
    let items = query_json(&db, &bounded, [id])?;
    Ok(Json(json!({key:items})))
}
fn query_json<P: rusqlite::Params>(
    db: &Connection,
    sql: &str,
    p: P,
) -> Result<Vec<Value>, ApiError> {
    let mut stmt = db.prepare(sql).map_err(ApiError::internal)?;
    let rows = stmt
        .query_map(p, |r| r.get::<_, String>(0))
        .map_err(ApiError::internal)?;
    rows.map(|x| parse_json(x.map_err(ApiError::internal)?))
        .collect()
}
fn query_cli_projects<P: rusqlite::Params>(
    db: &Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<c6_core::CliProjectSummary>, ApiError> {
    let mut statement = db.prepare(sql).map_err(ApiError::internal)?;
    let rows = statement
        .query_map(parameters, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(ApiError::internal)?;
    rows.map(|row| {
        let (
            id,
            workspace_id,
            slug,
            name,
            description,
            default_branch,
            head_sha,
            published_sha,
            role,
            updated_at,
        ) = row.map_err(ApiError::internal)?;
        Ok(c6_core::CliProjectSummary {
            id: Uuid::parse_str(&id).map_err(ApiError::internal)?,
            workspace_id: Uuid::parse_str(&workspace_id).map_err(ApiError::internal)?,
            slug,
            name,
            description,
            default_branch,
            head_sha,
            published_sha,
            role: parse_core_role(&role)?,
            updated_at: parse_time(&updated_at)?,
        })
    })
    .collect()
}

fn parse_core_role(role: &str) -> Result<c6_core::Role, ApiError> {
    match role {
        "consumer" => Ok(c6_core::Role::Consumer),
        "reader" => Ok(c6_core::Role::Reader),
        "runner" => Ok(c6_core::Role::Runner),
        "contributor" => Ok(c6_core::Role::Contributor),
        "maintainer" => Ok(c6_core::Role::Maintainer),
        "owner" => Ok(c6_core::Role::Owner),
        _ => Err(ApiError::internal("invalid persisted role")),
    }
}
fn parse_json(s: String) -> Result<Value, ApiError> {
    serde_json::from_str(&s).map_err(ApiError::internal)
}
fn audit(
    tx: &Transaction<'_>,
    actor: Option<&str>,
    action: &str,
    target_type: &str,
    target: Option<&str>,
    details: Value,
) -> Result<(), ApiError> {
    tx.execute(
        "INSERT INTO audit_events VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            Uuid::new_v4().to_string(),
            actor,
            action,
            target_type,
            target,
            details.to_string(),
            now()
        ],
    )
    .map_err(ApiError::internal)?;
    Ok(())
}
fn setting(db: &Connection, key: &str) -> Result<Option<String>, ApiError> {
    db.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
        r.get(0)
    })
    .optional()
    .map_err(ApiError::internal)
}
fn cookie(h: &HeaderMap, name: &str) -> Option<String> {
    h.get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|v| {
            let (k, val) = v.trim().split_once('=')?;
            (k == name).then(|| val.to_owned())
        })
}
fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn random_public_id() -> String {
    let mut bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn hash(v: &str) -> String {
    format!("{:x}", Sha256::digest(v.as_bytes()))
}
fn secure_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn parse_time(s: &str) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(ApiError::internal)
}
fn validate_credential_label(label: &str) -> Result<(), ApiError> {
    nonempty("label", label, 80)?;
    if label.chars().all(|character| !character.is_control()) {
        Ok(())
    } else {
        Err(ApiError::bad("label must contain printable characters"))
    }
}

fn validate_credential_restriction(
    state: &AppState,
    user_id: &str,
    restriction: Option<&c6_core::CredentialResourceRestriction>,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let Some(restriction) = restriction else {
        return Ok((None, None));
    };
    if restriction.workspace_id.is_none() && restriction.project_id.is_none() {
        return Err(ApiError::bad(
            "restriction must select a workspace or project",
        ));
    }
    let db = state.db.lock().map_err(ApiError::internal)?;
    let workspace_id = match (&restriction.workspace_id, &restriction.project_id) {
        (Some(workspace), None) => {
            let workspace = workspace.to_string();
            require_role_tx(&db, user_id, &workspace, "reader")?;
            workspace
        }
        (workspace, Some(project)) => {
            let project = project.to_string();
            let actual: Option<String> = db
                .query_row(
                    "SELECT p.workspace_id FROM projects p JOIN memberships m ON m.workspace_id=p.workspace_id WHERE p.id=?1 AND m.user_id=?2",
                    params![project, user_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(ApiError::internal)?;
            let actual = actual.ok_or_else(|| ApiError::not_found("project"))?;
            if workspace.is_some_and(|value| value.to_string() != actual.as_str()) {
                return Err(ApiError::bad("project is not in the restricted workspace"));
            }
            require_role_tx(&db, user_id, &actual, "reader")?;
            actual
        }
        (None, None) => unreachable!(),
    };
    Ok((
        Some(workspace_id),
        restriction.project_id.map(|project| project.to_string()),
    ))
}
fn role_rank(r: &str) -> u8 {
    match r {
        "consumer" => 0,
        "reader" => 1,
        "runner" => 2,
        "contributor" => 3,
        "maintainer" => 4,
        "owner" => 5,
        _ => 0,
    }
}
fn validate_role(r: &str) -> Result<(), ApiError> {
    if matches!(
        r,
        "consumer" | "reader" | "runner" | "contributor" | "maintainer" | "owner"
    ) {
        Ok(())
    } else {
        Err(ApiError::bad("invalid role"))
    }
}
fn nonempty(name: &str, v: &str, max: usize) -> Result<(), ApiError> {
    if v.trim().is_empty() || v.len() > max {
        Err(ApiError::bad(format!("{name} must be 1-{max} characters")))
    } else {
        Ok(())
    }
}
fn validate_slug(s: &str) -> Result<(), ApiError> {
    if s.len() >= 2
        && s.len() <= 63
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
    {
        Ok(())
    } else {
        Err(ApiError::bad(
            "slug must be 2-63 lowercase letters, digits, or hyphens",
        ))
    }
}
fn validate_identity(name: &str, label: &str, key: &str) -> Result<(), ApiError> {
    nonempty("displayName", name, 120)?;
    nonempty("deviceLabel", label, 120)?;
    if key.len() < 32 || key.len() > 8192 {
        return Err(ApiError::bad("publicKey must be 32-8192 characters"));
    }
    Ok(())
}
fn validate_project(i: &ProjectRequest) -> Result<(), ApiError> {
    validate_slug(&i.slug)?;
    nonempty("name", &i.name, 120)?;
    if i.description.len() > 4096 {
        return Err(ApiError::bad("description exceeds 4096 bytes"));
    }
    c6_git::validate_branch(&i.default_branch).map_err(map_git)
}
fn validate_secret_name(s: &str) -> Result<(), ApiError> {
    if s.len() <= 128
        && !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        Ok(())
    } else {
        Err(ApiError::bad(
            "secret name must contain only A-Z, 0-9, and underscore",
        ))
    }
}
fn map_conflict(e: rusqlite::Error) -> ApiError {
    if matches!(e,rusqlite::Error::SqliteFailure(ref x,_) if x.extended_code==2067||x.extended_code==1555)
    {
        ApiError::conflict("resource already exists")
    } else {
        ApiError::internal(e)
    }
}

fn map_git(error: c6_git::GitError) -> ApiError {
    match error {
        c6_git::GitError::NotFound(_) | c6_git::GitError::RevisionNotFound(_) => {
            ApiError::not_found("repository revision")
        }
        c6_git::GitError::InvalidRef(_)
        | c6_git::GitError::InvalidPath(_)
        | c6_git::GitError::LimitExceeded(_) => ApiError::bad(error.to_string()),
        _ => ApiError::internal(error),
    }
}

/// Rolls back only a repository whose name is a server-generated UUID and
/// whose canonical parent is the configured Git store. This is deliberately
/// narrower than a general repository deletion operation.
fn cleanup_new_repository(state: &AppState, id: &str) -> Result<(), ApiError> {
    Uuid::parse_str(id).map_err(ApiError::internal)?;
    let candidate = state.git_root.join(format!("{id}.git"));
    let metadata = std::fs::symlink_metadata(&candidate).map_err(ApiError::internal)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ApiError::internal("refusing unsafe repository cleanup"));
    }
    let canonical = candidate.canonicalize().map_err(ApiError::internal)?;
    if canonical.parent() != Some(state.git_root.as_path()) {
        return Err(ApiError::internal("repository cleanup escaped Git store"));
    }
    std::fs::remove_dir_all(canonical).map_err(ApiError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn test_state(dir: &TempDir) -> AppState {
        open_state_with_git_http(dir.path(), "http://127.0.0.1:8787".into(), false).unwrap()
    }
    fn git_test_state(dir: &TempDir) -> AppState {
        open_state_with_git_http(dir.path(), "http://127.0.0.1:8787".into(), true).unwrap()
    }
    async fn request(
        app: &Router,
        method: Method,
        path: &str,
        body: Value,
        headers: &[(&str, &str)],
    ) -> Response {
        let mut b = Request::builder()
            .method(method.clone())
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");
        if !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS)
            && !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("origin"))
        {
            b = b.header(header::ORIGIN, "http://127.0.0.1:8787");
        }
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        app.clone()
            .oneshot(b.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap()
    }
    async fn json_body(r: Response) -> Value {
        serde_json::from_slice(&to_bytes(r.into_body(), usize::MAX).await.unwrap()).unwrap()
    }
    fn bootstrap_token(state: &AppState) -> String {
        let token = "test-bootstrap".to_owned();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE settings SET value=?1 WHERE key='bootstrap_hash'",
                [hash(&token)],
            )
            .unwrap();
        token
    }
    async fn claim_owner(state: &AppState) -> (String, String) {
        let token = bootstrap_token(state);
        let r=request(&app(state.clone()),Method::POST,"/api/v1/bootstrap/claim",json!({"token":token,"displayName":"Owner","deviceLabel":"laptop","publicKey":"abcdefghijklmnopqrstuvwxyz0123456789"}),&[]).await;
        assert_eq!(r.status(), StatusCode::CREATED);
        auth_from_response(r).await
    }

    async fn auth_from_response(r: Response) -> (String, String) {
        let cookie = r
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().split(';').next().unwrap())
            .collect::<Vec<_>>()
            .join("; ");
        let j = json_body(r).await;
        (
            cookie,
            j["session"]["csrfToken"].as_str().unwrap().to_owned(),
        )
    }

    #[tokio::test]
    async fn health_and_status_are_public() {
        let d = TempDir::new().unwrap();
        let a = app(test_state(&d));
        assert_eq!(
            request(&a, Method::GET, "/healthz", json!(null), &[])
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(&a, Method::GET, "/api/v1/status", json!(null), &[])
                .await
                .status(),
            StatusCode::OK
        )
    }
    #[tokio::test]
    async fn unknown_api_is_json_404_and_api_responses_are_not_cacheable() {
        let d = TempDir::new().unwrap();
        let a = app(test_state(&d));
        for path in ["/api", "/api/", "/api/v1", "/api/v1/", "/api/v2/nope"] {
            let response = request(&a, Method::GET, path, json!(null), &[]).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert!(response.headers().contains_key("content-security-policy"));
            assert_eq!(json_body(response).await["error"]["code"], "not_found");
        }
    }
    #[tokio::test]
    async fn private_api_rejects_anonymous() {
        let d = TempDir::new().unwrap();
        let a = app(test_state(&d));
        let r = request(&a, Method::GET, "/api/v1/projects", json!(null), &[]).await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        let manifest = request(
            &a,
            Method::POST,
            "/api/v1/manifest/validate",
            json!({"source":"version = 1"}),
            &[],
        )
        .await;
        assert_eq!(manifest.status(), StatusCode::UNAUTHORIZED);
    }

    async fn issue_test_credential(
        app: &Router,
        cookies: &str,
        csrf: &str,
        kind: &str,
        scopes: Value,
        restriction: Value,
    ) -> Value {
        let response = request(
            app,
            Method::POST,
            "/api/v1/credentials",
            json!({"type":kind,"label":"test credential","scopes":scopes,"restriction":restriction}),
            &[("cookie", cookies), ("x-c6-csrf", csrf)],
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        json_body(response).await
    }

    #[tokio::test]
    async fn canonical_credential_request_rejects_unknown_and_untyped_fields() {
        let dir = TempDir::new().unwrap();
        let state = test_state(&dir);
        let (cookies, csrf) = claim_owner(&state).await;
        let application = app(state.clone());
        for invalid in [
            json!({"type":"cli","label":"strict","scopes":["api:read"],"unexpected":true}),
            json!({"type":"browser","label":"strict","scopes":["api:read"]}),
            json!({"type":"cli","label":"strict","scopes":["admin"]}),
            json!({
                "type":"cli",
                "label":"strict",
                "scopes":["api:read"],
                "restriction":{"workspaceId":Uuid::new_v4(),"unexpected":true}
            }),
        ] {
            let response = request(
                &application,
                Method::POST,
                "/api/v1/credentials",
                invalid,
                &[("cookie", &cookies), ("x-c6-csrf", &csrf)],
            )
            .await;
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        }
        let credential_count: i64 = state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM credentials", [], |row| row.get(0))
            .unwrap();
        assert_eq!(credential_count, 0);
    }

    #[tokio::test]
    async fn opaque_cli_credentials_are_verifier_only_and_class_separated() {
        let dir = TempDir::new().unwrap();
        let state = test_state(&dir);
        let (cookies, csrf) = claim_owner(&state).await;
        let application = app(state.clone());
        let issued = issue_test_credential(
            &application,
            &cookies,
            &csrf,
            "cli",
            json!(["api:read"]),
            Value::Null,
        )
        .await;
        let token = issued["token"].as_str().unwrap();
        assert!(token.starts_with("c6c_v1_"));
        let persisted: (String, String) = state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT public_id,verifier FROM credentials", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert!(!persisted.0.contains(token));
        assert!(!persisted.1.contains(token));
        assert_eq!(persisted.1, hash(token));

        let authorization = format!("Bearer {token}");
        let whoami = request(
            &application,
            Method::GET,
            "/api/v1/cli/whoami",
            json!(null),
            &[("authorization", &authorization)],
        )
        .await;
        assert_eq!(whoami.status(), StatusCode::OK);
        assert_eq!(json_body(whoami).await["user"]["displayName"], "Owner");

        for headers in [
            vec![("cookie", cookies.as_str())],
            vec![
                ("authorization", authorization.as_str()),
                ("cookie", cookies.as_str()),
            ],
            vec![(
                "authorization",
                "Bearer c6g_v1_AAAAAAAAAAAAAAAA_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            )],
        ] {
            assert_eq!(
                request(
                    &application,
                    Method::GET,
                    "/api/v1/cli/whoami",
                    json!(null),
                    &headers,
                )
                .await
                .status(),
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            request(
                &application,
                Method::GET,
                "/api/v1/cli/whoami",
                json!(null),
                &[
                    ("authorization", &authorization),
                    ("origin", "https://evil.example")
                ],
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );

        let listed = json_body(
            request(
                &application,
                Method::GET,
                "/api/v1/credentials",
                json!(null),
                &[("cookie", &cookies)],
            )
            .await,
        )
        .await;
        let serialized = listed.to_string();
        assert!(!serialized.contains(token));
        assert!(!serialized.contains(&persisted.1));
        assert_eq!(listed["credentials"][0]["type"], "cli");
        assert!(listed["credentials"][0]["userId"].is_string());
    }

    #[tokio::test]
    async fn credential_revocation_is_immediate_idempotent_and_restart_safe() {
        let dir = TempDir::new().unwrap();
        let state = test_state(&dir);
        let (cookies, csrf) = claim_owner(&state).await;
        let application = app(state.clone());
        let issued = issue_test_credential(
            &application,
            &cookies,
            &csrf,
            "cli",
            json!(["api:read"]),
            Value::Null,
        )
        .await;
        let id = issued["credential"]["id"].as_str().unwrap();
        let authorization = format!("Bearer {}", issued["token"].as_str().unwrap());
        for _ in 0..2 {
            assert_eq!(
                request(
                    &application,
                    Method::DELETE,
                    &format!("/api/v1/credentials/{id}"),
                    json!(null),
                    &[("cookie", &cookies), ("x-c6-csrf", &csrf)],
                )
                .await
                .status(),
                StatusCode::NO_CONTENT
            );
        }
        assert_eq!(
            request(
                &application,
                Method::GET,
                "/api/v1/cli/whoami",
                json!(null),
                &[("authorization", &authorization)],
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        drop(application);
        drop(state);
        assert_eq!(
            request(
                &app(test_state(&dir)),
                Method::GET,
                "/api/v1/cli/whoami",
                json!(null),
                &[("authorization", &authorization)],
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn expired_peer_revoked_and_device_revoked_credentials_fail_without_renewal() {
        // Expiry and peer revocation are checked before a CLI credential can
        // update last_used_at. Direct timestamps keep this regression
        // deterministic and avoid sleeping around a security boundary.
        let cli_dir = TempDir::new().unwrap();
        let cli_state = test_state(&cli_dir);
        let (cookies, csrf) = claim_owner(&cli_state).await;
        let cli_app = app(cli_state.clone());
        let expired = issue_test_credential(
            &cli_app,
            &cookies,
            &csrf,
            "cli",
            json!(["api:read"]),
            Value::Null,
        )
        .await;
        let expired_id = expired["credential"]["id"].as_str().unwrap();
        let expired_bearer = format!("Bearer {}", expired["token"].as_str().unwrap());
        cli_state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE credentials SET expires_at=?1 WHERE id=?2",
                params![(Utc::now() - Duration::minutes(1)).to_rfc3339(), expired_id],
            )
            .unwrap();
        assert_eq!(
            request(
                &cli_app,
                Method::GET,
                "/api/v1/cli/whoami",
                json!(null),
                &[("authorization", &expired_bearer)],
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        let expired_last_used: Option<String> = cli_state
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT last_used_at FROM credentials WHERE id=?1",
                [expired_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(expired_last_used.is_none());

        let peer_credential = issue_test_credential(
            &cli_app,
            &cookies,
            &csrf,
            "cli",
            json!(["api:read"]),
            Value::Null,
        )
        .await;
        let peer_credential_id = peer_credential["credential"]["id"].as_str().unwrap();
        let peer_bearer = format!("Bearer {}", peer_credential["token"].as_str().unwrap());
        let peer_id = peer_credential["credential"]["userId"].as_str().unwrap();
        cli_state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE users SET revoked_at=?1 WHERE id=?2",
                params![now(), peer_id],
            )
            .unwrap();
        assert_eq!(
            request(
                &cli_app,
                Method::GET,
                "/api/v1/cli/whoami",
                json!(null),
                &[("authorization", &peer_bearer)],
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        let peer_last_used: Option<String> = cli_state
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT last_used_at FROM credentials WHERE id=?1",
                [peer_credential_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(peer_last_used.is_none());

        // Device revocation is independently enforced for the Git credential
        // class. Cookie identity is not involved in this request.
        let git_dir = TempDir::new().unwrap();
        let git_state = git_test_state(&git_dir);
        let (git_cookies, git_csrf) = claim_owner(&git_state).await;
        let git_app = app(git_state.clone());
        let git_credential = issue_test_credential(
            &git_app,
            &git_cookies,
            &git_csrf,
            "git",
            json!(["git:read"]),
            Value::Null,
        )
        .await;
        let git_credential_id = git_credential["credential"]["id"].as_str().unwrap();
        let device_id = git_credential["credential"]["deviceId"].as_str().unwrap();
        let git_basic = format!(
            "Basic {}",
            STANDARD.encode(format!("c6:{}", git_credential["token"].as_str().unwrap()))
        );
        git_state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE devices SET revoked_at=?1 WHERE id=?2",
                params![now(), device_id],
            )
            .unwrap();
        let denied = request(
            &git_app,
            Method::GET,
            "/git/unknown/unknown.git/info/refs?service=git-upload-pack",
            json!(null),
            &[("authorization", &git_basic)],
        )
        .await;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        assert!(denied.headers().contains_key(header::WWW_AUTHENTICATE));
        let git_last_used: Option<String> = git_state
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT last_used_at FROM credentials WHERE id=?1",
                [git_credential_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(git_last_used.is_none());
    }

    #[tokio::test]
    async fn project_restriction_and_live_role_are_enforced_for_remote_discovery() {
        let dir = TempDir::new().unwrap();
        let state = test_state(&dir);
        let (cookies, csrf) = claim_owner(&state).await;
        let application = app(state.clone());
        let auth = [("cookie", cookies.as_str()), ("x-c6-csrf", csrf.as_str())];
        let mut projects = Vec::new();
        for (workspace_slug, project_slug) in [("first", "notes"), ("second", "code")] {
            let workspace = json_body(
                request(
                    &application,
                    Method::POST,
                    "/api/v1/workspaces",
                    json!({"slug":workspace_slug,"name":workspace_slug}),
                    &auth,
                )
                .await,
            )
            .await;
            let workspace_id = workspace["id"].as_str().unwrap().to_owned();
            let project = json_body(
                request(
                    &application,
                    Method::POST,
                    "/api/v1/projects",
                    json!({"workspaceId":workspace_id,"slug":project_slug,"name":project_slug}),
                    &auth,
                )
                .await,
            )
            .await;
            projects.push((workspace_id, project["id"].as_str().unwrap().to_owned()));
        }
        let issued = issue_test_credential(
            &application,
            &cookies,
            &csrf,
            "cli",
            json!(["api:read"]),
            json!({"workspaceId":projects[0].0,"projectId":projects[0].1}),
        )
        .await;
        let bearer = format!("Bearer {}", issued["token"].as_str().unwrap());
        let remote = request(
            &application,
            Method::GET,
            &format!("/api/v1/projects/{}/remote", projects[0].1),
            json!(null),
            &[("authorization", &bearer)],
        )
        .await;
        assert_eq!(remote.status(), StatusCode::OK);
        assert_eq!(json_body(remote).await["capabilities"]["fetch"], false);
        let git_issued = issue_test_credential(
            &application,
            &cookies,
            &csrf,
            "git",
            json!(["git:read"]),
            json!({"workspaceId":projects[0].0,"projectId":projects[0].1}),
        )
        .await;
        let git_basic = format!(
            "Basic {}",
            STANDARD.encode(format!("c6:{}", git_issued["token"].as_str().unwrap()))
        );
        assert_eq!(
            request(
                &application,
                Method::GET,
                "/git/first/notes.git/info/refs?service=git-upload-pack",
                json!(null),
                &[("authorization", &git_basic)],
            )
            .await
            .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            request(
                &application,
                Method::GET,
                &format!("/api/v1/projects/{}/remote", projects[1].1),
                json!(null),
                &[("authorization", &bearer)],
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let user_id: String = state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT user_id FROM credentials LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE memberships SET role='consumer' WHERE workspace_id=?1 AND user_id=?2",
                params![projects[0].0, user_id],
            )
            .unwrap();
        assert_eq!(
            request(
                &application,
                Method::GET,
                &format!("/api/v1/projects/{}/remote", projects[0].1),
                json!(null),
                &[("authorization", &bearer)],
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn git_http_is_basic_only_read_only_live_authorized_and_non_enumerating() {
        let dir = TempDir::new().unwrap();
        let state = git_test_state(&dir);
        let (cookies, csrf) = claim_owner(&state).await;
        let application = app(state.clone());
        let browser = [("cookie", cookies.as_str()), ("x-c6-csrf", csrf.as_str())];
        let mut projects = Vec::new();
        for (workspace_slug, project_slug) in [("alpha", "notes"), ("bravo", "code")] {
            let workspace = json_body(
                request(
                    &application,
                    Method::POST,
                    "/api/v1/workspaces",
                    json!({"slug":workspace_slug,"name":workspace_slug}),
                    &browser,
                )
                .await,
            )
            .await;
            let workspace_id = workspace["id"].as_str().unwrap().to_owned();
            let project = json_body(
                request(
                    &application,
                    Method::POST,
                    "/api/v1/projects",
                    json!({"workspaceId":workspace_id,"slug":project_slug,"name":project_slug}),
                    &browser,
                )
                .await,
            )
            .await;
            projects.push((
                workspace_slug,
                project_slug,
                workspace_id,
                project["id"].as_str().unwrap().to_owned(),
            ));
        }
        let issued = issue_test_credential(
            &application,
            &cookies,
            &csrf,
            "git",
            json!(["git:read"]),
            json!({"workspaceId":projects[0].2,"projectId":projects[0].3}),
        )
        .await;
        let credential_id = issued["credential"]["id"].as_str().unwrap().to_owned();
        let token = issued["token"].as_str().unwrap();
        let basic = format!("Basic {}", STANDARD.encode(format!("c6:{token}")));
        let first = format!(
            "/git/{}/{}.git/info/refs?service=git-upload-pack",
            projects[0].0, projects[0].1
        );
        let response = request(
            &application,
            Method::GET,
            &first,
            json!(null),
            &[("authorization", &basic)],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/x-git-upload-pack-advertisement"
        );
        assert!(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .starts_with(b"001e# service=git-upload-pack")
        );

        for headers in [
            vec![],
            vec![("cookie", cookies.as_str())],
            vec![
                ("authorization", basic.as_str()),
                ("cookie", cookies.as_str()),
            ],
            vec![("authorization", "Basic bm90LWM2Om5vdC1hLXRva2Vu")],
        ] {
            let denied = request(&application, Method::GET, &first, json!(null), &headers).await;
            assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
            assert!(denied.headers().contains_key(header::WWW_AUTHENTICATE));
        }
        let cross_workspace = format!(
            "/git/{}/{}.git/info/refs?service=git-upload-pack",
            projects[1].0, projects[1].1
        );
        assert_eq!(
            request(
                &application,
                Method::GET,
                &cross_workspace,
                json!(null),
                &[("authorization", &basic)],
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &application,
                Method::GET,
                &format!(
                    "/git/{}/{}.git/info/refs?service=git-receive-pack",
                    projects[0].0, projects[0].1
                ),
                json!(null),
                &[("authorization", &basic)],
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            request(
                &application,
                Method::POST,
                &format!(
                    "/git/{}/{}.git/git-receive-pack",
                    projects[0].0, projects[0].1
                ),
                json!(null),
                &[("authorization", &basic)],
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );

        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE memberships SET role='consumer' WHERE workspace_id=?1",
                [&projects[0].2],
            )
            .unwrap();
        assert_eq!(
            request(
                &application,
                Method::GET,
                &first,
                json!(null),
                &[("authorization", &basic)],
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE credentials SET revoked_at=?1 WHERE id=?2",
                params![now(), credential_id],
            )
            .unwrap();
        assert_eq!(
            request(
                &application,
                Method::GET,
                &first,
                json!(null),
                &[("authorization", &basic)],
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    #[tokio::test]
    async fn authenticated_peer_can_validate_manifest() {
        let d = TempDir::new().unwrap();
        let state = test_state(&d);
        let (cookies, _) = claim_owner(&state).await;
        let response = request(
            &app(state),
            Method::POST,
            "/api/v1/manifest/validate",
            json!({"source":"version = 1\n\n[build]\nstrategy = \"auto\""}),
            &[("cookie", &cookies)],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["valid"], true);
    }
    #[tokio::test]
    async fn claim_is_one_time_and_token_is_not_stored() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(d.path().join("bootstrap-token"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(d.path()).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let (c, csrf) = claim_owner(&s).await;
        assert!(!c.is_empty() && !csrf.is_empty());
        assert!(
            setting(&s.db.lock().unwrap(), "bootstrap_hash")
                .unwrap()
                .is_none()
        );
        assert!(!d.path().join("bootstrap-token").exists());
        let r=request(&app(s),Method::POST,"/api/v1/bootstrap/claim",json!({"token":"x","displayName":"Other","deviceLabel":"x","publicKey":"abcdefghijklmnopqrstuvwxyz0123456789"}),&[]).await;
        assert_eq!(r.status(), StatusCode::CONFLICT)
    }
    #[tokio::test]
    async fn uninitialized_database_reuses_secure_bootstrap_file() {
        let d = TempDir::new().unwrap();
        let token = "abcdefghijklmnopqrstuvwxyz0123456789_REUSE";
        write_bootstrap_token(&d.path().join("bootstrap-token"), token).unwrap();
        let state = test_state(&d);
        assert_eq!(
            setting(&state.db.lock().unwrap(), "bootstrap_hash").unwrap(),
            Some(hash(token))
        );
        let response = request(
            &app(state),
            Method::POST,
            "/api/v1/bootstrap/claim",
            json!({"token":token,"displayName":"Owner","deviceLabel":"laptop","publicKey":"abcdefghijklmnopqrstuvwxyz0123456789"}),
            &[],
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(!d.path().join("bootstrap-token").exists());
    }
    #[tokio::test]
    async fn cookie_mutations_require_csrf_and_same_origin() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (c, csrf) = claim_owner(&s).await;
        let a = app(s);
        assert_eq!(
            request(
                &a,
                Method::POST,
                "/api/v1/workspaces",
                json!({"slug":"team","name":"Team"}),
                &[("cookie", &c)]
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &a,
                Method::POST,
                "/api/v1/workspaces",
                json!({"slug":"team","name":"Team"}),
                &[
                    ("cookie", &c),
                    ("x-c6-csrf", &csrf),
                    ("origin", "https://evil.example")
                ]
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        let raw = app(test_state(&d)).oneshot(
            Request::builder().method(Method::POST).uri("/api/v1/invites/redeem")
                .header(header::CONTENT_TYPE,"application/json")
                .body(Body::from(json!({"token":"x","displayName":"X","deviceLabel":"X","publicKey":"abcdefghijklmnopqrstuvwxyz0123456789"}).to_string())).unwrap()
        ).await.unwrap();
        assert_eq!(raw.status(), StatusCode::FORBIDDEN);
    }
    #[tokio::test]
    async fn general_json_bodies_are_bounded() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (cookies, csrf) = claim_owner(&s).await;
        let response = request(
            &app(s),
            Method::POST,
            "/api/v1/workspaces",
            json!({"slug":"oversize","name":"x".repeat(70 * 1024)}),
            &[("cookie", &cookies), ("x-c6-csrf", &csrf)],
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
    #[tokio::test]
    async fn invite_is_single_use_and_restart_safe() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (c, csrf) = claim_owner(&s).await;
        let a = app(s.clone());
        let r = request(
            &a,
            Method::POST,
            "/api/v1/invites",
            json!({"role":"reader","expiresInMinutes":30}),
            &[("cookie", &c), ("x-c6-csrf", &csrf)],
        )
        .await;
        let invitation = json_body(r).await;
        let token = invitation["token"].as_str().unwrap().to_owned();
        let invite_url = invitation["inviteUrl"].as_str().unwrap();
        assert_eq!(
            invite_url,
            format!("http://127.0.0.1:8787/join#token={token}")
        );
        assert!(!invite_url.split('#').next().unwrap().contains(&token));
        drop(a);
        drop(s);
        let s2 = test_state(&d);
        let payload = json!({"token":token,"displayName":"Peer","deviceLabel":"phone","publicKey":"9876543210abcdefghijklmnopqrstuvwxyz"});
        assert_eq!(
            request(
                &app(s2.clone()),
                Method::POST,
                "/api/v1/invites/redeem",
                payload.clone(),
                &[]
            )
            .await
            .status(),
            StatusCode::CREATED
        );
        assert_eq!(
            request(
                &app(s2),
                Method::POST,
                "/api/v1/invites/redeem",
                payload,
                &[]
            )
            .await
            .status(),
            StatusCode::CONFLICT
        )
    }
    #[tokio::test]
    async fn csrf_cookie_survives_server_restart() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (cookies, csrf) = claim_owner(&s).await;
        assert!(cookies.contains("c6_session=") && cookies.contains("c6_csrf="));
        drop(s);
        let restarted = app(test_state(&d));
        assert_eq!(
            request(
                &restarted,
                Method::POST,
                "/api/v1/workspaces",
                json!({"slug":"restart-team","name":"Restart Team"}),
                &[("cookie", &cookies), ("x-c6-csrf", &csrf)],
            )
            .await
            .status(),
            StatusCode::CREATED
        );
    }
    #[tokio::test]
    async fn active_session_slides_but_expired_session_never_renews() {
        let d = TempDir::new().unwrap();
        let state = test_state(&d);
        let (cookies, _) = claim_owner(&state).await;
        let session_id: String = state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT id FROM sessions", [], |row| row.get(0))
            .unwrap();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE sessions SET expires_at=?1 WHERE id=?2",
                params![(Utc::now() + Duration::minutes(1)).to_rfc3339(), session_id],
            )
            .unwrap();
        let a = app(state.clone());
        let renewed = request(
            &a,
            Method::GET,
            "/api/v1/session",
            json!(null),
            &[("cookie", &cookies)],
        )
        .await;
        assert_eq!(renewed.status(), StatusCode::OK);
        let set_cookies = renewed
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(set_cookies.len(), 2);
        assert!(set_cookies[0].contains("c6_session=") && set_cookies[0].contains("HttpOnly"));
        assert!(set_cookies[1].contains("c6_csrf=") && !set_cookies[1].contains("HttpOnly"));
        assert!(set_cookies.iter().all(|cookie| {
            cookie.contains("SameSite=Strict")
                && cookie.contains(&format!("Max-Age={}", SESSION_HOURS * 3600))
        }));
        let expires: String = state
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT expires_at FROM sessions WHERE id=?1",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(parse_time(&expires).unwrap() > Utc::now() + Duration::days(29));

        let expired_at = (Utc::now() - Duration::minutes(1)).to_rfc3339();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE sessions SET expires_at=?1 WHERE id=?2",
                params![expired_at, session_id],
            )
            .unwrap();
        let expired = request(
            &a,
            Method::GET,
            "/api/v1/session",
            json!(null),
            &[("cookie", &cookies)],
        )
        .await;
        assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);
        assert!(!expired.headers().contains_key(header::SET_COOKIE));
        let persisted: String = state
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT expires_at FROM sessions WHERE id=?1",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted, expired_at);
    }
    #[tokio::test]
    async fn workspace_project_and_metadata_round_trip() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (c, csrf) = claim_owner(&s).await;
        let a = app(s);
        let h = [("cookie", c.as_str()), ("x-c6-csrf", csrf.as_str())];
        let r = request(
            &a,
            Method::POST,
            "/api/v1/workspaces",
            json!({"slug":"team","name":"Team"}),
            &h,
        )
        .await;
        let ws = json_body(r).await["id"].as_str().unwrap().to_owned();
        let r = request(
            &a,
            Method::POST,
            "/api/v1/projects",
            json!({"workspaceId":ws,"slug":"notes","name":"Notes"}),
            &h,
        )
        .await;
        assert_eq!(r.status(), StatusCode::CREATED);
        let id = json_body(r).await["id"].as_str().unwrap().to_owned();
        let repository_count = std::fs::read_dir(d.path().join("git")).unwrap().count();
        let duplicate = request(
            &a,
            Method::POST,
            "/api/v1/projects",
            json!({"workspaceId":ws,"slug":"notes","name":"Duplicate"}),
            &h,
        )
        .await;
        assert_eq!(duplicate.status(), StatusCode::CONFLICT);
        assert_eq!(
            std::fs::read_dir(d.path().join("git")).unwrap().count(),
            repository_count
        );
        let commits = request(
            &a,
            Method::GET,
            &format!("/api/v1/projects/{id}/repository/commits"),
            json!(null),
            &[("cookie", &c)],
        )
        .await;
        assert_eq!(commits.status(), StatusCode::OK);
        assert_eq!(
            json_body(commits).await["commits"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let tree = request(
            &a,
            Method::GET,
            &format!("/api/v1/projects/{id}/repository/tree"),
            json!(null),
            &[("cookie", &c)],
        )
        .await;
        assert_eq!(
            json_body(tree).await["entries"].as_array().unwrap().len(),
            2
        );
        let file = request(
            &a,
            Method::GET,
            &format!("/api/v1/projects/{id}/repository/files/README.md"),
            json!(null),
            &[("cookie", &c)],
        )
        .await;
        assert_eq!(file.status(), StatusCode::OK);
        assert!(
            to_bytes(file.into_body(), usize::MAX)
                .await
                .unwrap()
                .starts_with(b"# Notes")
        );
        for schedule in [
            json!({"job":"sync","cron":"not cron","timezone":"UTC"}),
            json!({"job":"sync","cron":"0 * * * *","timezone":"Mars/Olympus"}),
        ] {
            assert_eq!(
                request(
                    &a,
                    Method::POST,
                    &format!("/api/v1/projects/{id}/schedules"),
                    schedule,
                    &h,
                )
                .await
                .status(),
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            request(
                &a,
                Method::POST,
                &format!("/api/v1/projects/{id}/pull-requests"),
                json!({"title":"Change","sourceBranch":"main"}),
                &h
            )
            .await
            .status(),
            StatusCode::CREATED
        );
        assert_eq!(
            request(
                &a,
                Method::POST,
                &format!("/api/v1/projects/{id}/runs"),
                json!({"job":"sync","kind":"cron"}),
                &h
            )
            .await
            .status(),
            StatusCode::CREATED
        );
        assert_eq!(
            request(
                &a,
                Method::GET,
                &format!("/api/v1/projects/{id}/runs"),
                json!(null),
                &[("cookie", &c)]
            )
            .await
            .status(),
            StatusCode::OK
        )
    }
    #[tokio::test]
    async fn secret_values_are_explicitly_unavailable() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (c, csrf) = claim_owner(&s).await;
        let a = app(s);
        let h = [("cookie", c.as_str()), ("x-c6-csrf", csrf.as_str())];
        let ws = json_body(
            request(
                &a,
                Method::POST,
                "/api/v1/workspaces",
                json!({"slug":"team","name":"Team"}),
                &h,
            )
            .await,
        )
        .await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let id = json_body(
            request(
                &a,
                Method::POST,
                "/api/v1/projects",
                json!({"workspaceId":ws,"slug":"notes","name":"Notes"}),
                &h,
            )
            .await,
        )
        .await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            request(
                &a,
                Method::PUT,
                &format!("/api/v1/projects/{id}/secrets/API_KEY/value"),
                json!({"value":"nope"}),
                &h
            )
            .await
            .status(),
            StatusCode::NOT_IMPLEMENTED
        )
    }
    #[tokio::test]
    async fn revoked_session_stops_authorizing() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (c, csrf) = claim_owner(&s).await;
        let a = app(s);
        assert_eq!(
            request(
                &a,
                Method::DELETE,
                "/api/v1/session",
                json!(null),
                &[("cookie", &c), ("x-c6-csrf", &csrf)]
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            request(
                &a,
                Method::GET,
                "/api/v1/projects",
                json!(null),
                &[("cookie", &c)]
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        )
    }

    #[tokio::test]
    async fn workspace_roles_never_escalate_to_server_administrator() {
        let d = TempDir::new().unwrap();
        let s = test_state(&d);
        let (owner_cookies, owner_csrf) = claim_owner(&s).await;
        let a = app(s);
        let owner_headers = [
            ("cookie", owner_cookies.as_str()),
            ("x-c6-csrf", owner_csrf.as_str()),
        ];
        let owner_session = json_body(
            request(
                &a,
                Method::GET,
                "/api/v1/session",
                json!(null),
                &[("cookie", &owner_cookies)],
            )
            .await,
        )
        .await;
        assert_eq!(owner_session["serverAdministrator"], true);
        let workspace = json_body(
            request(
                &a,
                Method::POST,
                "/api/v1/workspaces",
                json!({"slug":"company","name":"Company"}),
                &owner_headers,
            )
            .await,
        )
        .await;
        let workspace_id = workspace["id"].as_str().unwrap();

        for (role, workspace_scope) in [("reader", None), ("owner", Some(workspace_id))] {
            let invite = json_body(
                request(
                    &a,
                    Method::POST,
                    "/api/v1/invites",
                    json!({"role":role,"workspaceId":workspace_scope}),
                    &owner_headers,
                )
                .await,
            )
            .await;
            let enrolled = request(
                &a,
                Method::POST,
                "/api/v1/invites/redeem",
                json!({
                    "token":invite["token"],
                    "displayName":format!("{role} peer"),
                    "deviceLabel":"browser",
                    "publicKey":format!("abcdefghijklmnopqrstuvwxyz0123456789-{role}")
                }),
                &[],
            )
            .await;
            assert_eq!(enrolled.status(), StatusCode::CREATED);
            let (cookies, csrf) = auth_from_response(enrolled).await;
            let mutating = [("cookie", cookies.as_str()), ("x-c6-csrf", csrf.as_str())];
            let peer_session = json_body(
                request(
                    &a,
                    Method::GET,
                    "/api/v1/session",
                    json!(null),
                    &[("cookie", &cookies)],
                )
                .await,
            )
            .await;
            assert_eq!(peer_session["serverAdministrator"], false);
            assert_eq!(
                request(
                    &a,
                    Method::POST,
                    "/api/v1/workspaces",
                    json!({"slug":format!("{role}-space"),"name":"Escalation"}),
                    &mutating,
                )
                .await
                .status(),
                StatusCode::FORBIDDEN
            );
            for endpoint in ["/api/v1/peers", "/api/v1/invites", "/api/v1/audit"] {
                assert_eq!(
                    request(
                        &a,
                        Method::GET,
                        endpoint,
                        json!(null),
                        &[("cookie", &cookies)]
                    )
                    .await
                    .status(),
                    StatusCode::FORBIDDEN
                );
            }
        }
    }

    #[test]
    fn non_loopback_plaintext_requires_explicit_opt_in() {
        assert!(
            validate_exposure("127.0.0.1".parse().unwrap(), "http://localhost:8787", false).is_ok()
        );
        assert!(
            validate_exposure(
                "0.0.0.0".parse().unwrap(),
                "http://laptop.local:8787",
                false
            )
            .is_err()
        );
        assert!(
            validate_exposure("0.0.0.0".parse().unwrap(), "http://laptop.local:8787", true).is_ok()
        );
        assert!(
            validate_exposure("0.0.0.0".parse().unwrap(), "https://c6.example", false).is_err()
        );
        assert!(validate_exposure("0.0.0.0".parse().unwrap(), "https://c6.example", true).is_ok());
    }

    #[test]
    fn git_http_transport_is_default_off_and_strictly_configured() {
        assert!(!parse_git_http_enabled(None).unwrap());
        assert!(!parse_git_http_enabled(Some("false")).unwrap());
        assert!(!parse_git_http_enabled(Some("0")).unwrap());
        assert!(parse_git_http_enabled(Some("true")).unwrap());
        assert!(parse_git_http_enabled(Some("1")).unwrap());
        for invalid in ["", "TRUE", "False", "yes", "on", " true", "true ", "2"] {
            assert!(
                parse_git_http_enabled(Some(invalid)).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn public_base_url_rejects_scheme_and_authority_bypasses() {
        for invalid in [
            "HTTP://localhost:8787",
            "ftp://localhost",
            "http://user:pass@localhost",
            "http://localhost/path",
            "http://localhost?next=https://evil.example",
            "http://localhost/#fragment",
            "not a url",
        ] {
            assert!(
                validate_public_base_url(invalid).is_err(),
                "accepted {invalid}"
            );
        }
        assert_eq!(
            validate_public_base_url("https://c6.example/").unwrap(),
            "https://c6.example"
        );
    }
    #[test]
    fn bootstrap_tokens_require_high_entropy_safe_encoding() {
        for invalid in [
            "x",
            "contains spaces but is still long enough",
            "abcdefghijklmnopqrstuvwxyz0123456789!bad",
        ] {
            assert!(validate_bootstrap_token(invalid).is_err());
        }
        assert!(validate_bootstrap_token("abcdefghijklmnopqrstuvwxyz0123456789_SAFE").is_ok());
    }
}

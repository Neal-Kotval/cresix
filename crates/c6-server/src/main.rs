use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::Context;
use axum::{
    Router,
    routing::{get, post},
};
use c6_core::{
    Action, Deployment, DeploymentStatus, Environment, Project, ProjectDetail, ProjectManifest,
    PullRequest, PullRequestStatus, Revision, Role, Run, RunKind, RunStatus, User, Workspace,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::RwLock};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    data: Arc<RwLock<DemoData>>,
}

#[derive(Clone)]
struct DemoData {
    user: User,
    workspace: Workspace,
    project: ProjectDetail,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    user: User,
    workspaces: Vec<Workspace>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateManifestRequest {
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateManifestResponse {
    valid: bool,
    manifest: Option<ProjectManifest>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunJobRequest {
    job: String,
    kind: RunKind,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "c6_server=info,tower_http=info".into()),
        )
        .init();

    let state = AppState {
        data: Arc::new(RwLock::new(DemoData::seed())),
    };
    let app = app(state);
    let port = std::env::var("C6_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8787);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(address).await.context("bind C6 server")?;
    info!(%address, "C6 is ready");
    axum::serve(listener, app).await.context("serve C6")?;
    Ok(())
}

fn app(state: AppState) -> Router {
    let api = Router::new()
        .route("/healthz", get(health))
        .route("/api/v1/session", get(session))
        .route("/api/v1/projects", get(list_projects))
        .route("/api/v1/projects/{slug}", get(project_detail))
        .route("/api/v1/projects/{slug}/runs", post(run_job))
        .route("/api/v1/projects/{slug}/publish", post(publish))
        .route("/api/v1/manifest/validate", post(validate_manifest))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let web_dist =
        PathBuf::from(std::env::var("C6_WEB_DIST").unwrap_or_else(|_| "web/dist".into()));
    api.fallback_service(
        ServeDir::new(&web_dist).fallback(ServeFile::new(web_dist.join("index.html"))),
    )
}

async fn health() -> axum::Json<HealthResponse> {
    axum::Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn session(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<SessionResponse> {
    let data = state.data.read().await;
    axum::Json(SessionResponse {
        user: data.user.clone(),
        workspaces: vec![data.workspace.clone()],
    })
}

async fn list_projects(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<Vec<Project>> {
    let data = state.data.read().await;
    axum::Json(vec![data.project.project.clone()])
}

async fn project_detail(
    axum::extract::Path(slug): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<ProjectDetail>, axum::http::StatusCode> {
    let data = state.data.read().await;
    if data.project.project.slug != slug {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }
    Ok(axum::Json(data.project.clone()))
}

async fn run_job(
    axum::extract::Path(slug): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(request): axum::Json<RunJobRequest>,
) -> Result<(axum::http::StatusCode, axum::Json<Run>), axum::http::StatusCode> {
    let mut data = state.data.write().await;
    if data.project.project.slug != slug {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }
    if !data.project.project.role.allows(Action::RunJob) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    let run = Run {
        id: Uuid::new_v4(),
        job: request.job,
        kind: request.kind,
        revision_sha: data
            .project
            .project
            .published_sha
            .clone()
            .unwrap_or_else(|| data.project.project.head_sha.clone()),
        status: RunStatus::Queued,
        trigger: format!("{} (manual)", data.user.handle),
        started_at: Utc::now(),
        finished_at: None,
    };
    data.project.runs.insert(0, run.clone());
    Ok((axum::http::StatusCode::ACCEPTED, axum::Json(run)))
}

async fn publish(
    axum::extract::Path(slug): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<(axum::http::StatusCode, axum::Json<Deployment>), axum::http::StatusCode> {
    let mut data = state.data.write().await;
    if data.project.project.slug != slug {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }
    if !data.project.project.role.allows(Action::Publish) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    let revision_sha = data.project.project.head_sha.clone();
    data.project.project.published_sha = Some(revision_sha.clone());
    let deployment = Deployment {
        id: Uuid::new_v4(),
        revision_sha,
        environment: Environment::Production,
        status: DeploymentStatus::Queued,
        url: data.project.project.app_url.clone(),
        created_at: Utc::now(),
    };
    data.project.deployments.insert(0, deployment.clone());
    Ok((axum::http::StatusCode::ACCEPTED, axum::Json(deployment)))
}

async fn validate_manifest(
    axum::Json(request): axum::Json<ValidateManifestRequest>,
) -> axum::Json<ValidateManifestResponse> {
    match ProjectManifest::parse(&request.source) {
        Ok(manifest) => axum::Json(ValidateManifestResponse {
            valid: true,
            manifest: Some(manifest),
            error: None,
        }),
        Err(error) => axum::Json(ValidateManifestResponse {
            valid: false,
            manifest: None,
            error: Some(error.to_string()),
        }),
    }
}

impl DemoData {
    fn seed() -> Self {
        let user = User {
            id: Uuid::new_v4(),
            handle: "neal".into(),
            display_name: "Neal Kotval".into(),
        };
        let workspace_id = Uuid::new_v4();
        let workspace = Workspace {
            id: workspace_id,
            slug: "paper-street".into(),
            name: "Paper Street".into(),
            role: Role::Owner,
        };
        let now = Utc::now();
        let preview = Deployment {
            id: Uuid::new_v4(),
            revision_sha: "e194d2a".into(),
            environment: Environment::Preview,
            status: DeploymentStatus::Ready,
            url: Some("https://pr-12.weeknote.c6.local".into()),
            created_at: now - Duration::minutes(18),
        };
        let project = Project {
            id: Uuid::new_v4(),
            workspace_id,
            slug: "weeknote".into(),
            name: "Weeknote".into(),
            description: "A tiny shared app that turns team activity into a Friday update.".into(),
            default_branch: "main".into(),
            head_sha: "7c1a840".into(),
            published_sha: Some("2fa39bd".into()),
            role: Role::Owner,
            app_url: Some("https://weeknote.c6.local".into()),
            updated_at: now - Duration::minutes(4),
        };
        let revisions = vec![
            Revision {
                sha: "7c1a840".into(),
                message: "Tighten the Friday summary prompt".into(),
                author: user.clone(),
                created_at: now - Duration::minutes(4),
            },
            Revision {
                sha: "2fa39bd".into(),
                message: "Add a private team dashboard".into(),
                author: user.clone(),
                created_at: now - Duration::days(1),
            },
            Revision {
                sha: "b0db109".into(),
                message: "Create Weeknote".into(),
                author: user.clone(),
                created_at: now - Duration::days(3),
            },
        ];
        let pull_requests = vec![PullRequest {
            number: 12,
            title: "Include decisions from project notes".into(),
            source_branch: "agent/friday-notes/0192".into(),
            target_branch: "main".into(),
            author: User {
                id: Uuid::new_v4(),
                handle: "friday-notes[bot]".into(),
                display_name: "Friday notes agent".into(),
            },
            status: PullRequestStatus::Open,
            preview: Some(preview.clone()),
            updated_at: now - Duration::minutes(18),
        }];
        let production = Deployment {
            id: Uuid::new_v4(),
            revision_sha: "2fa39bd".into(),
            environment: Environment::Production,
            status: DeploymentStatus::Ready,
            url: project.app_url.clone(),
            created_at: now - Duration::days(1),
        };
        let runs = vec![
            Run {
                id: Uuid::new_v4(),
                job: "friday-notes".into(),
                kind: RunKind::Agent,
                revision_sha: "2fa39bd".into(),
                status: RunStatus::Succeeded,
                trigger: "schedule · Fri 16:00".into(),
                started_at: now - Duration::days(7),
                finished_at: Some(now - Duration::days(7) + Duration::minutes(3)),
            },
            Run {
                id: Uuid::new_v4(),
                job: "sync-activity".into(),
                kind: RunKind::Cron,
                revision_sha: "2fa39bd".into(),
                status: RunStatus::Succeeded,
                trigger: "schedule · every hour".into(),
                started_at: now - Duration::minutes(26),
                finished_at: Some(now - Duration::minutes(25)),
            },
        ];
        Self {
            user,
            workspace,
            project: ProjectDetail {
                project,
                readme: "# Weeknote\n\nA small private app for making the weekly update less painful. It gathers activity, proposes a summary, and lets the team edit before sharing.\n\n## What runs\n\n- `web` serves the shared editor\n- `sync-activity` refreshes source data hourly\n- `friday-notes` uses Codex to propose the weekly summary and opens a pull request".into(),
                revisions,
                pull_requests,
                deployments: vec![production, preview],
                runs,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_is_public_and_stable() {
        let response = app(AppState {
            data: Arc::new(RwLock::new(DemoData::seed())),
        })
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_project_is_not_found() {
        let response = app(AppState {
            data: Arc::new(RwLock::new(DemoData::seed())),
        })
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }
}

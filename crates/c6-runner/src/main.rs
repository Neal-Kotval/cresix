//! C6 local runner daemon.
//!
//! This process is a privilege boundary even though the current simulation
//! backend has no elevated privileges. Never add Docker socket access to the
//! control-plane process.

use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use c6_runner::{
    Authenticator, DaemonConfig, FileResultStore, RunnerService, SimulationBackend,
    load_or_create_auth_key, serve,
};
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "c6_runner=info".into()),
        )
        .init();

    let socket_path = PathBuf::from(
        std::env::var("C6_RUNNER_SOCKET").unwrap_or_else(|_| "/tmp/c6-runner.sock".into()),
    );
    let state_dir = PathBuf::from(
        std::env::var("C6_RUNNER_STATE_DIR").unwrap_or_else(|_| "/tmp/c6-runner-state".into()),
    );
    let auth_key = match std::env::var("C6_RUNNER_AUTH_KEY") {
        Ok(key) => key.into_bytes(),
        Err(std::env::VarError::NotPresent) => {
            let key_path = std::env::var("C6_RUNNER_AUTH_KEY_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    socket_path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join("runner.key")
                });
            load_or_create_auth_key(&key_path).context("load or create runner key file")?
        }
        Err(error) => return Err(error).context("C6_RUNNER_AUTH_KEY must be valid UTF-8"),
    };
    let authenticator = Authenticator::new(auth_key).context("invalid runner key")?;
    let store = Arc::new(
        FileResultStore::new(state_dir)
            .await
            .context("initialize runner result store")?,
    );
    let service = Arc::new(RunnerService::new(
        authenticator,
        Arc::new(SimulationBackend),
        store,
    ));
    let daemon = serve(
        DaemonConfig {
            socket_path: socket_path.clone(),
        },
        service,
    );

    info!(path = %socket_path.display(), backend = "simulation", "runner boundary is ready");
    tokio::select! {
        result = daemon => result.context("serve runner")?,
        result = signal::ctrl_c() => result.context("listen for shutdown")?,
    }
    Ok(())
}

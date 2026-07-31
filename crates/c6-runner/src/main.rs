//! Privileged execution boundary.
//!
//! The MVP runner intentionally starts as a separate daemon before it gains
//! container lifecycle privileges. The control plane must never acquire direct
//! Docker access as execution support is added.

use std::path::PathBuf;

use anyhow::Context;
use tokio::{io::AsyncWriteExt, net::UnixListener, signal};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    let socket = PathBuf::from(
        std::env::var("C6_RUNNER_SOCKET").unwrap_or_else(|_| "/tmp/c6-runner.sock".into()),
    );
    if socket.exists() {
        std::fs::remove_file(&socket).context("remove stale runner socket")?;
    }
    let listener = UnixListener::bind(&socket).context("bind runner socket")?;
    info!(path = %socket.display(), "runner boundary is ready");

    loop {
        tokio::select! {
            connection = listener.accept() => {
                let (mut stream, _) = connection.context("accept runner connection")?;
                stream.write_all(b"{\"status\":\"ready\",\"capabilities\":[]}\n").await?;
            }
            _ = signal::ctrl_c() => break,
        }
    }
    let _ = std::fs::remove_file(socket);
    Ok(())
}

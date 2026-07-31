use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use anyhow::{Context, bail};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "c6_cloud=info,tower_http=info".into()),
        )
        .init();

    let bind: IpAddr = std::env::var("C6_CLOUD_BIND")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()
        .context("C6_CLOUD_BIND must be an IP address")?;
    let port = std::env::var("C6_CLOUD_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8790);
    let public_origin = std::env::var("C6_CLOUD_PUBLIC_ORIGIN")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{port}"));
    if !bind.is_loopback() {
        bail!(
            "the dogfood Cloud service is loopback-only; public hosting requires production account enrollment and TLS ingress not implemented in this revision"
        );
    }

    let config = c6_cloud::Config {
        data_dir: PathBuf::from(
            std::env::var("C6_CLOUD_DATA_DIR").unwrap_or_else(|_| ".c6-cloud".into()),
        ),
        public_origin,
        web_dir: PathBuf::from(
            std::env::var("C6_CLOUD_WEB_DIR").unwrap_or_else(|_| "cloud-web/dist".into()),
        ),
    };
    let cloud = c6_cloud::Cloud::open(config)?;
    let address = SocketAddr::new(bind, port);
    let listener = TcpListener::bind(address).await.context("bind C6 Cloud")?;
    info!(%address, "Cresix Cloud dogfood service is ready");
    axum::serve(
        listener,
        c6_cloud::app(cloud).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("serve C6 Cloud")
}

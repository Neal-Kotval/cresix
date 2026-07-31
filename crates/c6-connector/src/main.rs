use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, bail};
use c6_connector::{
    LoadedConfig,
    catalog::{CatalogError, publish_snapshot, run_periodic},
    runtime::run_reconnecting,
};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();
    let path = parse_args()?;
    let config = Arc::new(LoadedConfig::load(&path).context("load connector configuration")?);

    match publish_snapshot(&config).await {
        Ok(accepted) => info!(
            projects = accepted.accepted_projects,
            "published local project catalog"
        ),
        Err(
            CatalogError::LocalAuthenticationRejected | CatalogError::CloudAuthenticationRejected,
        ) => {
            bail!("catalog authentication was rejected; rotate or replace the affected credential")
        }
        Err(error) => {
            warn!(error = %error, "initial catalog publication failed; relay connection will still be attempted")
        }
    }
    info!(installation_id = %config.config.installation_id, "starting outbound Cresix connector");
    tokio::select! {
        relay = run_reconnecting(config.clone()) => relay.context("connector stopped"),
        catalog = run_periodic(&config) => {
            catalog.context("periodic catalog publication stopped")?;
            unreachable!("the periodic catalog loop returns only on authentication rejection")
        }
    }
}

fn parse_args() -> anyhow::Result<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    let Some(flag) = args.next() else {
        bail!("usage: c6-connector --config <owner-only-file>");
    };
    let Some(path) = args.next() else {
        bail!("usage: c6-connector --config <owner-only-file>");
    };
    if flag != "--config" || args.next().is_some() {
        bail!("usage: c6-connector --config <owner-only-file>");
    }
    Ok(path.into())
}

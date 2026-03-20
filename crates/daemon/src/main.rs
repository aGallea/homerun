use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = homerund::config::Config::default();
    config.ensure_dirs()?;

    tracing::info!("HomeRun daemon starting...");
    homerund::server::serve(config).await
}

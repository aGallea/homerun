use anyhow::Result;
use homerund::logging::{DaemonLogLayer, DaemonLogState};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let mut config = homerund::config::Config::default();
    let config_path = config.config_path();
    if config_path.exists() {
        match homerund::config::Config::load(&config_path) {
            Ok(saved) => config = saved,
            Err(e) => tracing::warn!("Failed to load config.toml, using defaults: {}", e),
        }
    }
    config.ensure_dirs()?;

    let daemon_log_state = DaemonLogState::new(&config.log_dir());
    let runtime = tokio::runtime::Handle::current();

    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let daemon_layer = DaemonLogLayer::new(daemon_log_state.clone(), runtime);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(daemon_layer)
        .init();

    tracing::info!("HomeRun daemon starting...");
    homerund::server::serve(config, daemon_log_state).await
}

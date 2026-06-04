//! DrafftInk relay server binary.

use drafftink_server::config::ServerConfig;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "drafftink_server=info,tower_http=info".into()),
        )
        .init();

    let config = ServerConfig::from_env();
    info!("Store: {:?}", config.store);
    if let Err(e) = drafftink_server::run(config).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}

mod config;
mod handlers;
mod models;
mod routes;

use std::{net::SocketAddr, sync::Arc};

use config::Config;
use eventsdb::sqlite::SqliteEventLog;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::from_env()?;
    if let Some(parent) = config.db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let log = Arc::new(SqliteEventLog::open(&config.db_path).await?);
    let app = routes::router(log);
    let address = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = TcpListener::bind(address).await?;

    info!(port = config.port, db_path = %config.db_path.display(), "starting logbook");
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("logbook=info,tower_http=info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

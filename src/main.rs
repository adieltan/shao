mod api;
mod collector;
mod config;
mod db;
mod dockge;
mod docker;
mod immich;
mod sensors;

use api::{create_router, AppState};
use clap::Parser;
use collector::CollectorService;
use config::Config;
use db::Database;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "shao", author = "Adiel Tan", version = "0.1.0", about = "Shao (哨兵) - Ultra-lightweight Linux server sentinel in Rust")]
struct Args {
    /// Path to config.toml file
    #[arg(short, long)]
    config: Option<String>,

    /// Override listening port
    #[arg(short, long)]
    port: Option<u16>,

    /// Override listening host
    #[arg(long)]
    host: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let mut config = Config::load_or_default(args.config.as_deref());
    if let Some(p) = args.port {
        config.server.port = p;
    }
    if let Some(h) = args.host {
        config.server.host = h;
    }

    println!(r#"
   _____ _                 
  / ____| |                
 | (___ | |__   __ _  ___  
  \___ \| '_ \ / _` |/ _ \ 
  ____) | | | | (_| | (_) |
 |_____/|_| |_|\__,_|\___/  (哨兵 - Sentinel v0.1.0)
"#);

    info!("🛡️ Initializing Shao (哨兵) Server Sentinel...");

    let db = Database::new(&config.server.db_path)
        .expect("Failed to initialize embedded SQLite database");
    info!("📁 Database initialized at {}", config.server.db_path);

    let latest_telemetry = Arc::new(RwLock::new(None));
    let (tx, _rx) = broadcast::channel(100);

    let state = AppState {
        config: config.clone(),
        db: db.clone(),
        latest: latest_telemetry.clone(),
        tx: tx.clone(),
    };

    let collector = CollectorService::new(config.clone(), db, latest_telemetry, tx);
    tokio::spawn(async move {
        collector.run().await;
    });

    let app = create_router(state);
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("🚀 Shao Sentinel Web UI online at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("🛑 Shao Sentinel gracefully stopped.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

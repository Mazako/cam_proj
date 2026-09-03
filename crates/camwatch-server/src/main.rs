use std::path::PathBuf;

use axum::serve;
use camwatch::config::Config;
use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use camwatch_server::{
    app_state::AppState,
    router::{router, validate_bind_address},
};

#[derive(Debug, Parser)]
#[command(name = "camwatch-server", version, about = "Camwatch SSR server")]
struct Cli {
    #[arg(long, env = "CAMWATCH_CONFIG", default_value = "camwatch.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    init_logging();

    let cli = Cli::parse();
    let config = match Config::load(&cli.config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Configuration error: {error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = validate_bind_address(config.app.bind_address) {
        eprintln!("Server configuration error: {error}");
        std::process::exit(2);
    }

    let bind_address = config.app.bind_address;
    let listener = match TcpListener::bind(bind_address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Server bind error: {error}");
            std::process::exit(3);
        }
    };

    tracing::info!(%bind_address, "server started");

    if let Err(error) = serve(listener, router(AppState::default())).await {
        eprintln!("Server error: {error}");
        std::process::exit(3);
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

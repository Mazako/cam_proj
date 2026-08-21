mod config;

use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[derive(Debug, Parser)]
#[command(name = "camwatch", version, about = "Local network camera monitoring")]
struct Cli {
    #[arg(long, env = "CAMWATCH_CONFIG", default_value = "camwatch.toml")]
    config: PathBuf,
}

fn main() {
    init_logging();

    let cli = Cli::parse();
    let config = match Config::load(&cli.config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Configuration error: {error}");
            std::process::exit(2);
        }
    };

    tracing::info!(
        camera_count = config.cameras.len(),
        bind_address = %config.app.bind_address,
        "configuration loaded"
    );
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

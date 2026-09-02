use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "camwatch", version, about = "Local network camera monitoring")]
pub(crate) struct Cli {
    #[arg(long, env = "CAMWATCH_CONFIG", default_value = "camwatch.toml")]
    pub(crate) config: PathBuf,
}

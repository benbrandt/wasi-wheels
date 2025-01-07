//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use clap::{Parser, Subcommand};
use wasi_wheels::download_and_compile_cpython;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Prepares the necessary Cpython and WASI SDK tooling for building the tools.
    InstallBuildTools,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::InstallBuildTools => download_and_compile_cpython().await?,
    }

    Ok(())
}

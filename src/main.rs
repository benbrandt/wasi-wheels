//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use wasi_wheels::{build, download_sdist, install_build_tools, SupportedProjects};

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
    /// Download the sdist package for the specified project and version
    Sdist {
        /// The project (package) you want to download
        project: String,
        /// Which released version you want to download
        release_version: String,
        /// Where to download. Defaults to "sdist" directory in current directory
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
    /// Build a given package into a WASI wheel.
    Build {
        /// The project (package) you want to download
        project: SupportedProjects,
        /// Which released version you want to download
        release_version: String,
        /// Where to download. Defaults to "sdist" directory in current directory
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::InstallBuildTools => install_build_tools().await,
        Commands::Sdist {
            project,
            release_version,
            output_dir,
        } => download_sdist(&project, &release_version, output_dir).await,
        Commands::Build {
            project,
            release_version,
            output_dir,
        } => build(project, &release_version, output_dir).await,
    }
}

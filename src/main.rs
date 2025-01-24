//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use wasi_wheels::{
    build_and_publish, download_package, install_build_tools, PythonVersion, SupportedProjects,
};

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
    DownloadPackage {
        /// The project (package) you want to download
        project: String,
        /// Which released version you want to download
        release_version: String,
        /// Where to download. Defaults to "packages" directory in current directory
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
    /// Build a given package into a WASI wheel.
    Build {
        /// The project (package) you want to download
        project: SupportedProjects,
        /// Which released version you want to download
        release_version: String,
        /// Where to download. Defaults to "packages" directory in current directory
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
        /// Optionally publish the wheel as a release in GitHub
        #[arg(long)]
        publish: bool,
        /// Python versions to build with. Defaults to all supported versions
        #[arg(long, value_enum, default_values_t=[PythonVersion::Py3_12, PythonVersion::Py3_13])]
        python_versions: Vec<PythonVersion>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::InstallBuildTools => install_build_tools().await,
        Commands::DownloadPackage {
            project,
            release_version,
            output_dir,
        } => download_package(&project, &release_version, output_dir).await,
        Commands::Build {
            project,
            release_version,
            output_dir,
            publish,
            python_versions,
        } => {
            build_and_publish(
                project,
                &release_version,
                output_dir,
                &python_versions,
                publish,
            )
            .await
        }
    }
}

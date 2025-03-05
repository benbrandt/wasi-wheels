//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use wasi_wheels::{
    PythonVersion, SupportedProjects, build_and_publish, download_package, generate_index,
    install_build_tools,
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
        /// Optionally publish the wheel as a release in GitHub
        #[command(flatten)]
        publish_flags: PublishFlags,
        /// Python versions to build with. Defaults to all supported versions
        #[arg(long, value_enum, default_values_t=[PythonVersion::Py3_12, PythonVersion::Py3_13])]
        python_versions: Vec<PythonVersion>,
    },
    /// Generate a Python Package Index for a given repo
    GenerateIndex {
        /// Which repository this is being released for: <user>/<repo>
        repo: String,
        /// Where to download. Defaults to "index" directory in current directory
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

#[derive(Args, Debug)]
#[group(requires = "publish")]
struct PublishFlags {
    /// Which repository this is being released for: <user>/<repo>
    #[arg(long)]
    repo: Option<String>,
    /// Which run id was used when building in CI
    #[arg(long)]
    run_id: Option<usize>,
}

impl PublishFlags {
    /// Generate info about the run context
    fn run_info(&self) -> String {
        match (self.repo.as_ref(), self.run_id) {
            (None | Some(_), None) => "No provided run information".to_string(),
            (None, Some(run_id)) => format!("Built with run {run_id}"),
            (Some(repo), Some(run_id)) => {
                format!("Built with run https://github.com/{repo}/actions/runs/{run_id}")
            }
        }
    }
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
        } => {
            download_package(&project, &release_version, output_dir).await?;
            Ok(())
        }
        Commands::Build {
            project,
            release_version,
            output_dir,
            publish,
            python_versions,
            publish_flags,
        } => {
            build_and_publish(
                project,
                &release_version,
                output_dir,
                &python_versions,
                publish.then(|| publish_flags.run_info()),
            )
            .await
        }
        Commands::GenerateIndex { repo, output_dir } => {
            let (owner, repo) = repo
                .split_once('/')
                .ok_or_else(|| anyhow::anyhow!("Invalid repo argument of type <owner>/<repo>"))?;
            generate_index(owner, repo, output_dir).await
        }
    }
}

use std::{env, path::PathBuf, sync::LazyLock};

use build_tools::{download_and_compile_cpython, download_wasi_sdk};
use clap::ValueEnum;
use tokio::process::Command;

use crate::run;

mod build_tools;
mod pydantic;

/// Currently supported Python version
const PYTHON_VERSION: &str = "3.12";
/// Current directory of this repository
pub static REPO_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()));
/// Directory for storing package sdist folders
pub static PACKAGES_DIR: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("packages"));
/// Directory the Wasi SDK should be setup at
static WASI_SDK: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("wasi-sdk"));

/// Directory Cpython should be setup at
fn cpython_dir(python_version: &str) -> PathBuf {
    REPO_DIR.join(format!("cpython-{python_version}"))
}

/// Downloads and prepares the WASI-SDK for use in compilation steps.
/// Downloads and compiles a fork of Python 3.12 that can be compiled to WASI for use with componentize-py
///
/// # Errors
/// Will error if the repo cannot be downloaded or compilation fails
///
/// # Panics
/// If certain paths are invalid because of failed download
pub async fn install_build_tools() -> anyhow::Result<()> {
    // Make sure WASI SDK is available
    download_wasi_sdk().await?;
    download_and_compile_cpython(PYTHON_VERSION).await
}

/// Projects that we support builds for
#[derive(Debug, Clone, Copy, ValueEnum, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum SupportedProjects {
    /// <https://pypi.org/project/pydantic-core/>
    PydanticCore,
}

/// Build a given package into a WASI wheel.
///
/// # Errors
/// If the build fails.
pub async fn build(
    project: SupportedProjects,
    release_version: &str,
    output_dir: Option<PathBuf>,
    publish: bool,
) -> anyhow::Result<()> {
    let wheel_path = match project {
        SupportedProjects::PydanticCore => pydantic::build(release_version, output_dir).await?,
    };
    if publish {
        publish_release(project, release_version, wheel_path).await?;
    }
    Ok(())
}

async fn publish_release(
    project: SupportedProjects,
    release_version: &str,
    wheel_path: PathBuf,
) -> anyhow::Result<()> {
    let tag = format!("{project}-{release_version}");
    run(Command::new("gh").args([
        "release",
        "create",
        &tag,
        wheel_path.to_str().unwrap(),
        "--title",
        &tag,
        "--notes",
        &format!("Generated using `wasi-wheels build {project} {release_version} --publish`"),
    ]))
    .await
}

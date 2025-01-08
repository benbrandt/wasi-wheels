use std::{env, path::PathBuf, sync::LazyLock};

use build_tools::{download_and_compile_cpython, download_wasi_sdk};
use clap::ValueEnum;

mod build_tools;
mod pydantic;

/// Currently supported Python version
const PYTHON_VERSION: &str = "3.12";
/// Current directory of this repository
pub static REPO_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()));
/// Directory the Wasi SDK should be setup at
static WASI_SDK: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("wasi-sdk"));
/// Directory Cpython should be setup at
static CPYTHON: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("cpython"));

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
    download_and_compile_cpython().await
}

/// Projects that we support builds for
#[derive(Debug, Clone, Copy, ValueEnum)]
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
) -> anyhow::Result<()> {
    match project {
        SupportedProjects::PydanticCore => pydantic::build(release_version, output_dir).await,
    }
}

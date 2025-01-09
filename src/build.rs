use std::{env, ffi::OsStr, path::PathBuf, sync::LazyLock};

use build_tools::download_wasi_sdk;
use clap::ValueEnum;
use strum::IntoEnumIterator;
use tokio::process::Command;

use crate::run;

mod build_tools;
mod pydantic;

pub use build_tools::PythonVersion;

/// Current directory of this repository
pub static REPO_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()));
/// Directory for storing package sdist folders
pub static PACKAGES_DIR: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("packages"));
/// Directory the Wasi SDK should be setup at
static WASI_SDK: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("wasi-sdk"));

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
    for python_version in PythonVersion::iter() {
        python_version.download_and_compile_cpython().await?;
    }
    Ok(())
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
    python_versions: &[PythonVersion],
    publish: bool,
) -> anyhow::Result<()> {
    let mut wheel_paths = vec![];

    for python_version in python_versions {
        let wheel_path = match project {
            SupportedProjects::PydanticCore => {
                pydantic::build(*python_version, release_version, output_dir.clone()).await?
            }
        };
        wheel_paths.push(wheel_path);
    }

    if publish {
        publish_release(project, release_version, &wheel_paths).await?;
    }

    Ok(())
}

async fn publish_release(
    project: SupportedProjects,
    release_version: &str,
    wheel_paths: &[PathBuf],
) -> anyhow::Result<()> {
    let tag = format!("{project}-{release_version}");
    let notes =
        format!("Generated using `wasi-wheels build {project} {release_version} --publish`");

    run(Command::new("gh").args(
        [
            "release", "create", &tag, "--title", &tag, "--notes", &notes,
        ]
        .into_iter()
        .map(OsStr::new)
        .chain(wheel_paths.iter().map(|p| p.as_os_str())),
    ))
    .await
}

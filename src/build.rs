use std::{env, ffi::OsStr, iter::once, path::PathBuf, sync::LazyLock};

use clap::ValueEnum;
use sha2::{Digest, Sha256};
use strum::IntoEnumIterator;
use tokio::{fs, process::Command};

use crate::run;

mod build_tools;
mod pydantic;
mod wheels;

pub use build_tools::PythonVersion;

/// Current directory of this repository
pub static REPO_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()));
/// Directory for storing package sdist folders
pub static PACKAGES_DIR: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("packages"));
/// Directory for storing package index files
pub static INDEX_DIR: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("index"));

/// Downloads and prepares the WASI-SDK for use in compilation steps.
/// Downloads and compiles a fork of Python 3.12 that can be compiled to WASI for use with componentize-py
///
/// # Errors
/// Will error if the repo cannot be downloaded or compilation fails
///
/// # Panics
/// If certain paths are invalid because of failed download
pub async fn install_build_tools() -> anyhow::Result<()> {
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
pub async fn build_and_publish(
    project: SupportedProjects,
    release_version: &str,
    output_dir: Option<PathBuf>,
    python_versions: &[PythonVersion],
    publish_notes: Option<String>,
) -> anyhow::Result<()> {
    let wheel_paths = build(project, release_version, output_dir, python_versions).await?;

    if let Some(notes) = publish_notes {
        publish_release(project, release_version, &wheel_paths, &notes).await?;
    }

    Ok(())
}

pub async fn build(
    project: SupportedProjects,
    release_version: &str,
    output_dir: Option<PathBuf>,
    python_versions: &[PythonVersion],
) -> anyhow::Result<Vec<PathBuf>> {
    let mut wheel_paths = vec![];

    for python_version in python_versions {
        let wheel_path = match project {
            SupportedProjects::PydanticCore => {
                pydantic::build(*python_version, release_version, output_dir.clone()).await?
            }
        };
        wheel_paths.push(wheel_path);
    }

    Ok(wheel_paths)
}

async fn publish_release(
    project: SupportedProjects,
    release_version: &str,
    wheel_paths: &[PathBuf],
    notes: &str,
) -> anyhow::Result<()> {
    let tag = format!("{project}/v{release_version}");

    let hashes = generate_hashes(wheel_paths).await?;
    let temp_dir = tempfile::tempdir()?;
    let hashes_path = temp_dir.path().join("hashes.txt");
    fs::write(&hashes_path, hashes).await?;

    run(Command::new("gh").args(
        ["release", "create", &tag, "--title", &tag, "--notes", notes]
            .into_iter()
            .map(OsStr::new)
            .chain(wheel_paths.iter().map(|p| p.as_os_str()))
            .chain(once(hashes_path.as_os_str())),
    ))
    .await
}

async fn generate_hashes(wheel_paths: &[PathBuf]) -> anyhow::Result<String> {
    let mut hashes = String::new();
    for wheel_path in wheel_paths {
        let content = fs::read(wheel_path).await?;
        let hash = format!("{:x}", Sha256::digest(&content));
        let filename = wheel_path.file_name().unwrap().to_string_lossy();
        hashes.push_str(&format!("{filename}\t{hash}\n"));
    }

    Ok(hashes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_project_display() -> anyhow::Result<()> {
        let wheel_paths = build(
            SupportedProjects::PydanticCore,
            "2.27.2",
            None,
            &[PythonVersion::Py3_12],
        )
        .await?;

        let hashes = generate_hashes(&wheel_paths).await?;

        for path in wheel_paths {
            assert!(hashes.contains(path.file_name().unwrap().to_str().unwrap()));
        }

        Ok(())
    }
}

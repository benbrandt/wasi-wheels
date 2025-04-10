use std::path::PathBuf;

use tokio::process::Command;

use crate::{
    SupportedProjects,
    build::{
        build_tools::PythonVersion,
        wheels::{default_wheel_flags, wheel_path},
    },
    download_package, run,
};

use super::wheels::retag_wheel;

/// Builds Pydantic and returns the wheel path for publishing
pub async fn build(
    python_version: PythonVersion,
    version: &str,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let package_dir = download_package("regex", version, output_dir).await?;
    let path_variable = python_version.create_venv(&package_dir).await?;

    let wheel = wheel_path(
        SupportedProjects::Regex,
        python_version,
        &package_dir,
        version,
    );
    if !wheel.exists() {
        run(Command::new("pip")
            .args(["install", "build", "--upgrade"])
            // Make it possible to not have to activate the venv
            .env("PATH", &path_variable))
        .await?;

        run(default_wheel_flags(
            Command::new("python").args(["-m", "build", "--wheel"]),
            python_version,
            &package_dir,
            &path_variable,
        ))
        .await?;

        retag_wheel(
            SupportedProjects::Regex,
            python_version,
            package_dir,
            version,
            &path_variable,
        )
        .await?;
    }

    Ok(wheel)
}

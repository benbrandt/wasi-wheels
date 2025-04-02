use std::path::PathBuf;

use tokio::process::Command;

use crate::{
    SupportedProjects,
    build::{
        build_tools::PythonVersion,
        wheels::{default_wheel_flags, retag_any_wheel, wheel_path},
    },
    download_package, run,
};

/// Builds Pydantic and returns the wheel path for publishing
pub async fn build(
    python_version: PythonVersion,
    version: &str,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    const RUST_TARGET: &str = "wasm32-wasip1";
    let package_dir = download_package("pydantic-core", version, output_dir).await?;
    let wasi_sdk_path = python_version.wasi_sdk_path();
    let path_variable = python_version.create_venv(&package_dir).await?;

    let wheel = wheel_path(
        SupportedProjects::PydanticCore,
        python_version,
        &package_dir,
        version,
    );
    if !wheel.exists() {
        run(Command::new("pip")
            .args(["install", "typing-extensions", "maturin", "--upgrade"])
            // Make it possible to not have to activate the venv
            .env("PATH", &path_variable))
        .await?;

        run(default_wheel_flags(Command::new("maturin").args([
            "build",
            "--release",
            "--target",
            RUST_TARGET,
            "--out",
            "dist",
            "-i",
            &format!("python{python_version}"),
            "--strip"
        ]), python_version, &package_dir, &path_variable)
            .env("PYO3_CROSS_LIB_DIR", python_version.cross_lib_dir())
            .env("RUSTFLAGS",  format!("-C link-args=-L{wasi_sdk}/share/wasi-sysroot/lib/{RUST_TARGET}/ -C link-self-contained=no -C link-args=--experimental-pic -C link-args=--shared -C relocation-model=pic -C linker-plugin-lto=yes -C opt-level=s -C lto=true -C codegen-units=1", wasi_sdk = wasi_sdk_path.to_str().unwrap()))
            .env("CARGO_BUILD_TARGET", RUST_TARGET)
        )
        .await?;

        // Rewrite the wheel to the correct target
        retag_any_wheel(
            SupportedProjects::PydanticCore,
            python_version,
            package_dir,
            version,
            &path_variable,
        )
        .await?;
    }

    Ok(wheel)
}

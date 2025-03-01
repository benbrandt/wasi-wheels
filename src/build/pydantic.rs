use std::{env, path::PathBuf};

use tokio::process::Command;

use crate::{build::build_tools::PythonVersion, download_package, run};

use super::PACKAGES_DIR;

/// Builds Pydantic and returns the wheel path for publishing
pub async fn build(
    python_version: PythonVersion,
    version: &str,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    const PLATFORM_TAG: &str = "wasi_0_0_0_wasm32";
    const RUST_TARGET: &str = "wasm32-wasip1";
    let output_dir = output_dir.unwrap_or_else(|| PACKAGES_DIR.clone());
    let package_dir = output_dir.join(format!("pydantic_core-{version}"));
    download_package("pydantic-core", version, Some(output_dir)).await?;
    let wasi_sdk_path = python_version.wasi_sdk_path();

    let venv_dir = package_dir.join(format!(".venv-{python_version}"));
    let path = format!(
        "{}:{}",
        venv_dir.join("bin").to_str().unwrap(),
        env::var("PATH").unwrap_or_default()
    );

    if !venv_dir.exists() {
        run(Command::new(format!("python{python_version}"))
            .args(["-m", "venv", venv_dir.to_str().unwrap()])
            .current_dir(&package_dir))
        .await?;
    }

    let wheel = package_dir.join(format!(
        "dist/pydantic_core-{version}-cp{py_version}-cp{py_version}-{PLATFORM_TAG}.whl",
        py_version = python_version.to_string().replace('.', "")
    ));
    if !wheel.exists() {
        run(Command::new("pip")
            .args(["install", "typing-extensions", "maturin", "--upgrade"])
            // Make it possible to not have to activate the venv
            .env("PATH", &path))
        .await?;

        let cross_prefix = python_version.cross_prefix();
        let cc = wasi_sdk_path.join("bin/clang");

        run(Command::new("maturin").args([
            "build",
            "--release",
            "--target",
            RUST_TARGET,
            "--out",
            "dist",
            "-i",
            &format!("python{python_version}"),
        ])
            .current_dir(&package_dir)
            // Make it possible to not have to activate the venv
            .env("PATH", &path)
            .env("CROSS_PREFIX", &cross_prefix)
            .env("WASI_SDK_PATH", &wasi_sdk_path)
            .env("PYO3_CROSS_LIB_DIR", python_version.cross_lib_dir())
            .env("SYSCONFIG", python_version.cross_lib_dir())
            .env("CC", &cc)
            .env("CXX", wasi_sdk_path.join("bin/clang++"))
            .env(
                "PYTHONPATH",
                cross_prefix.join(format!("lib/python{python_version}")),
            )
            .env("RUSTFLAGS",  format!("-C link-args=-L{wasi_sdk}/share/wasi-sysroot/lib/{RUST_TARGET}/ -C linker={wasi_sdk}/bin/wasm-ld -C link-self-contained=no -C link-args=--experimental-pic -C link-args=--shared -C relocation-model=pic -C linker-plugin-lto=yes", wasi_sdk = wasi_sdk_path.to_str().unwrap()))
            .env("CFLAGS", format!("-I{}/include/python{python_version} -D__EMSCRIPTEN__=1", cross_prefix.to_str().unwrap()))
            .env("CXXFLAGS", format!("-I{}/include/python{python_version}", cross_prefix.to_str().unwrap()))
            .env("LDSHARED", cc)
            .env("AR", wasi_sdk_path.join("bin/ar"))
            .env("RANLIB", "true")
            .env("LDFLAGS", "-shared")
            .env("_PYTHON_SYSCONFIGDATA_NAME", "_sysconfigdata__wasi_wasm32-wasi")
            .env("CARGO_BUILD_TARGET", RUST_TARGET)
        )
        .await?;

        // Rewrite the wheel to the correct target
        run(Command::new("pip")
            .args(["install", "wheel", "--upgrade"])
            // Make it possible to not have to activate the venv
            .env("PATH", &path))
        .await?;
        run(Command::new("wheel")
            .args([
                "tags",
                "--platform-tag",
                PLATFORM_TAG,
                "--remove",
                // Maturin outputs a wheel with `any` platform tag
                &wheel.to_str().unwrap().replace(PLATFORM_TAG, "any"),
            ])
            // Make it possible to not have to activate the venv
            .env("PATH", &path))
        .await?;
    }

    Ok(wheel)
}

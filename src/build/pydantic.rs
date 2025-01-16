use std::path::PathBuf;

use tokio::{fs, process::Command};

use crate::{build::build_tools::PythonVersion, download_package, run};

use super::{PACKAGES_DIR, WASI_SDK};

/// Builds Pydantic and returns the wheel path for publishing
pub async fn build(
    python_version: PythonVersion,
    version: &str,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let output_dir = output_dir.unwrap_or_else(|| PACKAGES_DIR.clone());
    let package_dir = output_dir.join(format!("pydantic_core-{version}"));
    download_package("pydantic-core", version, Some(output_dir)).await?;

    if package_dir.join(".venv").exists() {
        fs::remove_dir_all(package_dir.join(".venv")).await?;
    }

    run(Command::new(format!("python{python_version}"))
        .args(["-m", "venv", ".venv"])
        .current_dir(&package_dir))
    .await?;

    let wheel = package_dir.join(format!(
        "dist/pydantic_core-{version}-cp{py_version}-cp{py_version}-any.whl",
        py_version = python_version.to_string().replace('.', "")
    ));
    if !wheel.exists() {
        let cross_prefix = python_version.cross_prefix();
        let cc = WASI_SDK.join("bin/clang");
        let rust_target = "wasm32-wasip1";

        let tempdir = tempfile::tempdir()?;
        let script_path = tempdir.path().join("run_build.sh");
        fs::write(
            &script_path,
            format!(
                "#!/bin/bash
set -eou pipefail
. .venv/bin/activate
pip install typing-extensions maturin
maturin build --release --target wasm32-wasip1 --out dist -i python{python_version}"
            ),
        )
        .await?;

        run(Command::new("bash")
            .arg(script_path)
            .current_dir(&package_dir)
            .env("CROSS_PREFIX", &cross_prefix)
            .env("WASI_SDK_PATH", &*WASI_SDK)
            .env("PYO3_CROSS_LIB_DIR", python_version.cross_lib_dir())
            .env("SYSCONFIG", python_version.cross_lib_dir())
            .env("CC", &cc)
            .env("CXX", WASI_SDK.join("bin/clang++"))
            .env(
                "PYTHONPATH",
                cross_prefix.join(format!("lib/python{python_version}")),
            )
            .env("RUSTFLAGS",  format!("-C link-args=-L{wasi_sdk}/share/wasi-sysroot/lib/wasm32-wasip2/ -C linker={wasi_sdk}/bin/wasm-ld -C link-self-contained=no -C link-args=--experimental-pic -C link-args=--shared -C relocation-model=pic -C linker-plugin-lto=yes", wasi_sdk = WASI_SDK.to_str().unwrap()))
            .env("CFLAGS", format!("-I{}/include/python{python_version} -D__EMSCRIPTEN__=1", cross_prefix.to_str().unwrap()))
            .env("CXXFLAGS", format!("-I{}/include/python{python_version}", cross_prefix.to_str().unwrap()))
            .env("LDSHARED", cc)
            .env("AR", WASI_SDK.join("bin/ar"))
            .env("RANLIB", "true")
            .env("LDFLAGS", "-shared")
            .env("_PYTHON_SYSCONFIGDATA_NAME", "_sysconfigdata__wasi_wasm32-wasi")
            .env("CARGO_BUILD_TARGET", rust_target)
        ).await?;
    }

    Ok(wheel)
}

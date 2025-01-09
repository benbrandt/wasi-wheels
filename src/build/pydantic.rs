use std::path::PathBuf;

use tokio::{fs, process::Command};

use crate::{download_package, run};

use super::{CPYTHON, PACKAGES_DIR, PYTHON_VERSION, WASI_SDK};

/// Builds Pydantic and returns the wheel path for publishing
pub async fn build(version: &str, output_dir: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let output_dir = output_dir.unwrap_or_else(|| PACKAGES_DIR.clone());
    let package_dir = output_dir.join(format!("pydantic_core-{version}"));
    download_package("pydantic-core", version, Some(output_dir)).await?;

    if !package_dir.join(".venv").exists() {
        run(Command::new(format!("python{PYTHON_VERSION}"))
            .args(["-m", "venv", ".venv"])
            .current_dir(&package_dir))
        .await?;
    }

    let wheel = package_dir.join(format!(
        "dist/pydantic_core-{version}-cp{py_version}-cp{py_version}-any.whl",
        py_version = PYTHON_VERSION.replace('.', "")
    ));
    if !wheel.exists() {
        let cpython_wasi_dir = CPYTHON.join("builddir/wasi");
        let cross_prefix = cpython_wasi_dir.join("install");
        let lib_wasi = cpython_wasi_dir.join(format!("build/lib.wasi-wasm32-{PYTHON_VERSION}"));
        let cc = WASI_SDK.join("bin/clang");
        let rust_target = "wasm32-wasip1";

        fs::write(
            package_dir.join("run_build.sh"),
            "#!/bin/bash
    set -eou pipefail
    . .venv/bin/activate
    pip install typing-extensions maturin
    maturin build --release --target wasm32-wasip1 --out dist -i python3.12",
        )
        .await?;

        run(Command::new("bash")
            .arg("./run_build.sh")
            .current_dir(&package_dir)
            .env("CROSS_PREFIX", &cross_prefix)
            .env("WASI_SDK_PATH", &*WASI_SDK)
            .env("PYO3_CROSS_LIB_DIR", &lib_wasi)
            .env("SYSCONFIG", lib_wasi)
            .env("CC", &cc)
            .env("CXX", WASI_SDK.join("bin/clang++"))
            .env(
                "PYTHONPATH",
                cross_prefix.join(format!("lib/python{PYTHON_VERSION}")),
            )
            .env("RUSTFLAGS",  format!("-C link-args=-L{wasi_sdk}/share/wasi-sysroot/lib/wasm32-wasi/ -C linker={wasi_sdk}/bin/wasm-ld -C link-self-contained=no -C link-args=--experimental-pic -C link-args=--shared -C relocation-model=pic -C linker-plugin-lto=yes", wasi_sdk = WASI_SDK.to_str().unwrap()))
            .env("CFLAGS", format!("-I{}/include/python{PYTHON_VERSION} -D__EMSCRIPTEN__=1", cross_prefix.to_str().unwrap()))
            .env("CXXFLAGS", format!("-I{}/include/python{PYTHON_VERSION}", cross_prefix.to_str().unwrap()))
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

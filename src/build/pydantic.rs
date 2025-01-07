use tokio::{fs, process::Command};

use crate::{run, run_inherit, REPO_DIR};

use super::{CPYTHON, WASI_SDK};

async fn build() -> anyhow::Result<()> {
    const PYTHON_VERSION: &str = "3.12";
    let package_dir = REPO_DIR.join("sdist/pydantic_core-2.27.1");

    if !package_dir.join(".venv").exists() {
        run(Command::new("python3.12")
            .args(["-m", "venv", ".venv"])
            .current_dir(&package_dir))
        .await?;
    }

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

    run_inherit(Command::new("bash")
        .arg("./run_build.sh")
        .current_dir(package_dir)
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::download_sdist;

    use super::*;

    #[tokio::test]
    async fn can_build_pydantic() -> anyhow::Result<()> {
        download_sdist("pydantic-core", "2.27.1", None).await?;
        build().await
    }
}

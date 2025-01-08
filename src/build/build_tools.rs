use std::env;

use flate2::bufread::GzDecoder;
use tar::Archive;
use tokio::{fs, process::Command};

use crate::{
    build::{CPYTHON, PYTHON_VERSION, WASI_SDK},
    run, REPO_DIR,
};

/// Downloads and prepares the WASI-SDK for use in compilation steps
///
/// # Errors
/// Will error if WASI SDK cannot be downloaded, or if called on an unsupported OS or Architecture.
pub async fn download_wasi_sdk() -> anyhow::Result<()> {
    const WASI_SDK_RELEASE: &str = "wasi-sdk-25";
    const WASI_SDK_VERSION: &str = "25.0";

    if !WASI_SDK.exists() {
        let arch = match env::consts::ARCH {
            arch @ "x86_64" => arch,
            "aarch64" => "arm64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture")),
        };
        let os @ ("linux" | "macos" | "windows") = env::consts::OS else {
            return Err(anyhow::anyhow!("Unsupported OS"));
        };

        let download_dir = format!("wasi-sdk-{WASI_SDK_VERSION}-{arch}-{os}");
        let bytes = reqwest::get(format!("https://github.com/WebAssembly/wasi-sdk/releases/download/{WASI_SDK_RELEASE}/{download_dir}.tar.gz"))
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        tokio::task::spawn_blocking(move || {
            Archive::new(GzDecoder::new(&bytes[..])).unpack(REPO_DIR.as_path())
        })
        .await??;
        fs::rename(download_dir, &*WASI_SDK).await?;

        // Hack for cpython to use wasip2 files. Uses wasip2 for wasi
        let sysroot_path = WASI_SDK.join("share").join("wasi-sysroot");
        for dir in ["include", "lib", "share"] {
            let dir = sysroot_path.join(dir);
            fs::rename(dir.join("wasm32-wasi"), dir.join("wasm32-wasi-bk")).await?;
            fs::rename(dir.join("wasm32-wasip2"), dir.join("wasm32-wasi")).await?;
        }
    }

    Ok(())
}

/// Downloads and compiles a fork of Python 3.12 that can be compiled to WASI for use with componentize-py
///
/// # Errors
/// Will error if the repo cannot be downloaded or compilation fails
///
/// # Panics
/// If certain paths are invalid because of failed download
pub async fn download_and_compile_cpython() -> anyhow::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    const PYTHON_EXECUTABLE: &str = "python.exe";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    const PYTHON_EXECUTABLE: &str = "python";
    const GITHUB_USER: &str = "benbrandt";
    const GITHUB_REPO: &str = "cpython";
    const GITHUB_BRANCH: &str = "3.12-wasi";

    if !CPYTHON.exists() {
        let bytes = reqwest::get(
            format!("https://github.com/{GITHUB_USER}/{GITHUB_REPO}/archive/refs/heads/{GITHUB_BRANCH}.tar.gz"),
        )
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        tokio::task::spawn_blocking(move || {
            Archive::new(GzDecoder::new(&bytes[..])).unpack(REPO_DIR.as_path())
        })
        .await??;
        fs::rename(format!("{GITHUB_REPO}-{GITHUB_BRANCH}"), &*CPYTHON).await?;
    }

    let cpython_wasi_dir = CPYTHON.join("builddir/wasi");
    let cpython_native_dir = CPYTHON.join("builddir/build");
    if !cpython_wasi_dir
        .join(format!("libpython{PYTHON_VERSION}.so"))
        .exists()
        && !cpython_wasi_dir
            .join(format!("libpython{PYTHON_VERSION}.a"))
            .exists()
    {
        if !cpython_native_dir.join(PYTHON_EXECUTABLE).exists() {
            fs::create_dir_all(&cpython_native_dir).await?;
            fs::create_dir_all(&cpython_wasi_dir).await?;

            run(Command::new("../../configure")
                .current_dir(&cpython_native_dir)
                .arg(format!(
                    "--prefix={}/install",
                    cpython_native_dir.to_str().unwrap()
                )))
            .await?;

            run(Command::new("make").current_dir(&cpython_native_dir)).await?;
        }

        let config_guess = String::from_utf8(
            Command::new("../../config.guess")
                .current_dir(&cpython_wasi_dir)
                .output()
                .await?
                .stdout,
        )?;

        run(Command::new("../../Tools/wasm/wasi-env")
            .env("WASI_SDK_PATH", WASI_SDK.to_str().unwrap())
            .env("CONFIG_SITE", "../../Tools/wasm/config.site-wasm32-wasi")
            .env("CFLAGS", "-fPIC")
            .current_dir(&cpython_wasi_dir)
            .args([
                "../../configure",
                "-C",
                "--host=wasm32-unknown-wasi",
                &format!("--build={config_guess}"),
                &format!(
                    "--with-build-python={}/{PYTHON_EXECUTABLE}",
                    cpython_native_dir.to_str().unwrap()
                ),
                &format!("--prefix={}/install", cpython_wasi_dir.to_str().unwrap()),
                "--enable-wasm-dynamic-linking",
                "--enable-ipv6",
                "--disable-test-modules",
            ]))
        .await?;

        run(Command::new("make")
            .current_dir(&cpython_wasi_dir)
            .arg("install"))
        .await?;
    }

    Ok(())
}

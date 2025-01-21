use std::{env, path::PathBuf};

use clap::ValueEnum;
use flate2::bufread::GzDecoder;
use reqwest::{Client, IntoUrl};
use strum::EnumIter;
use tar::Archive;
use tokio::{fs, process::Command};

use crate::run;

use super::{REPO_DIR, WASI_SDK};

/// Currently supported Python versions
#[derive(Clone, Copy, Debug, EnumIter, ValueEnum)]
pub enum PythonVersion {
    /// Python 3.12
    Py3_12,
    /// Python 3.13
    Py3_13,
}

impl std::fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PythonVersion::Py3_12 => write!(f, "3.12"),
            PythonVersion::Py3_13 => write!(f, "3.13"),
        }
    }
}

impl PythonVersion {
    const GITHUB_USER: &str = "benbrandt";
    const GITHUB_REPO: &str = "cpython";

    /// What the current, exact version being used is
    fn current_patch_version(self) -> &'static str {
        match self {
            Self::Py3_12 => "3.12.8",
            Self::Py3_13 => "3.13.1",
        }
    }

    /// Directory Cpython should be setup at
    pub fn cpython_dir(self) -> PathBuf {
        REPO_DIR.join(format!("cpython-{}", self.current_patch_version()))
    }

    /// Directory for the wasi install directory
    #[must_use]
    pub fn cross_prefix(self) -> PathBuf {
        self.wasi_dir().join("install")
    }

    /// Directory to find the lib files for wasi
    #[must_use]
    pub fn cross_lib_dir(self) -> PathBuf {
        self.wasi_dir()
            .join(format!("build/lib.wasi-wasm32-{self}"))
    }

    fn wasi_dir(self) -> PathBuf {
        self.cpython_dir().join(match self {
            PythonVersion::Py3_12 => "builddir/wasi",
            PythonVersion::Py3_13 => "cross-build/wasm32-wasip1",
        })
    }

    /// Which GitHub branch should be used for downloading
    fn github_branch(self) -> String {
        format!("{}-wasi", self.current_patch_version())
    }

    /// Downloads and compiles a fork of `CPython` that can be compiled to WASI for use with componentize-py
    ///
    /// # Errors
    /// Will error if the repo cannot be downloaded or compilation fails
    ///
    /// # Panics
    /// If certain paths are invalid because of failed download
    pub async fn download_and_compile_cpython(self) -> anyhow::Result<()> {
        match self {
            PythonVersion::Py3_12 => self.download_and_compile_legacy().await,
            PythonVersion::Py3_13 => self.download_and_compile_with_wasi_script().await,
        }
    }

    /// Downloads and compiles a fork of `CPython` that can be compiled to WASI for use with componentize-py
    /// Uses the newer wasi.py script to do it, if available.
    ///
    /// # Errors
    /// Will error if the repo cannot be downloaded or compilation fails
    ///
    /// # Panics
    /// If certain paths are invalid because of failed download
    async fn download_and_compile_with_wasi_script(self) -> anyhow::Result<()> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        const PYTHON_EXECUTABLE: &str = "python.exe";
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        const PYTHON_EXECUTABLE: &str = "python";
        const HOST_TRIPLE: &str = "wasm32-wasip1";
        let github_branch = self.github_branch();
        let cpython = self.cpython_dir();

        if !cpython.exists() {
            let bytes = get_bytes(format!(
                "https://github.com/{}/{}/archive/refs/heads/{github_branch}.tar.gz",
                Self::GITHUB_USER,
                Self::GITHUB_REPO
            ))
            .await?;
            tokio::task::spawn_blocking(move || {
                Archive::new(GzDecoder::new(&bytes[..])).unpack(REPO_DIR.as_path())
            })
            .await??;
            fs::rename(format!("{}-{github_branch}", Self::GITHUB_REPO), &cpython).await?;
        }

        let cpython_wasi_dir = cpython.join(format!("cross-build/{HOST_TRIPLE}"));
        let cpython_native_dir = cpython.join("cross-build/build");

        if !cpython_wasi_dir.join(format!("libpython{self}.a")).exists() {
            if !cpython_native_dir.join(PYTHON_EXECUTABLE).exists() {
                run(Command::new("python3")
                    .env("WASI_SDK_PATH", WASI_SDK.to_str().unwrap())
                    .current_dir(&cpython)
                    .args([
                        "./Tools/wasm/wasi.py",
                        "configure-build-python",
                        "--",
                        "--config-cache",
                    ]))
                .await?;

                run(Command::new("python3")
                    .env("WASI_SDK_PATH", WASI_SDK.to_str().unwrap())
                    .current_dir(&cpython)
                    .args(["./Tools/wasm/wasi.py", "make-build-python"]))
                .await?;
            }

            run(Command::new("python3")
                .env("WASI_SDK_PATH", WASI_SDK.to_str().unwrap())
                .current_dir(&cpython)
                .args([
                    "./Tools/wasm/wasi.py",
                    "configure-host",
                    &format!("--host-triple={HOST_TRIPLE}"),
                    // Current script doesn't work for some reason...
                    "--host-runner=echo",
                    "--",
                    "--config-cache",
                    &format!("--prefix={}/install", cpython_wasi_dir.to_str().unwrap()),
                    "--enable-wasm-dynamic-linking",
                    "--enable-ipv6",
                    "--disable-test-modules",
                ]))
            .await?;

            run(Command::new("python3")
                .env("WASI_SDK_PATH", WASI_SDK.to_str().unwrap())
                .current_dir(&cpython)
                .args([
                    "./Tools/wasm/wasi.py",
                    "make-host",
                    &format!("--host-triple={HOST_TRIPLE}"),
                ]))
            .await?;

            run(Command::new("make")
                .current_dir(&cpython_wasi_dir)
                .arg("install"))
            .await?;
        }

        Ok(())
    }

    async fn download_and_compile_legacy(self) -> anyhow::Result<()> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        const PYTHON_EXECUTABLE: &str = "python.exe";
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        const PYTHON_EXECUTABLE: &str = "python";
        let github_branch = self.github_branch();
        let cpython = self.cpython_dir();

        if !cpython.exists() {
            let bytes = get_bytes(format!(
                "https://github.com/{}/{}/archive/refs/heads/{github_branch}.tar.gz",
                Self::GITHUB_USER,
                Self::GITHUB_REPO
            ))
            .await?;
            tokio::task::spawn_blocking(move || {
                Archive::new(GzDecoder::new(&bytes[..])).unpack(REPO_DIR.as_path())
            })
            .await??;
            fs::rename(format!("{}-{github_branch}", Self::GITHUB_REPO), &cpython).await?;
        }

        let cpython_wasi_dir = cpython.join("builddir/wasi");
        let cpython_native_dir = cpython.join("builddir/build");
        if !cpython_wasi_dir.join(format!("libpython{self}.a")).exists() {
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
}

async fn get_bytes(url: impl IntoUrl) -> anyhow::Result<bytes::Bytes> {
    let bytes = Client::builder()
        .use_rustls_tls()
        .build()?
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    Ok(bytes)
}

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
        let bytes = get_bytes(format!("https://github.com/WebAssembly/wasi-sdk/releases/download/{WASI_SDK_RELEASE}/{download_dir}.tar.gz")).await?;

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
            run(Command::new("cp")
                .args(["-r", "wasm32-wasip2", "wasm32-wasi"])
                .current_dir(dir))
            .await?;
        }
    }

    Ok(())
}

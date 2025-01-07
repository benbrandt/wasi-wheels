//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use std::{env, fs, iter, path::PathBuf, process::Command, sync::LazyLock};

use anyhow::{bail, Context};
use flate2::bufread::GzDecoder;
use tar::Archive;

/// Run a given command with common error handling behavior
///
/// # Errors
///
/// Returns error if the command fails for any reason.
pub fn run(command: &mut Command) -> anyhow::Result<Vec<u8>> {
    let command_string = iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let output = command.output().with_context({
        let command_string = command_string.clone();
        move || command_string
    })?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        bail!(
            "command `{command_string}` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Current directory of this repository
pub static REPO_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()));
/// Directory the Wasi SDK should be setup at
pub static WASI_SDK: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("wasi-sdk"));
/// Directory Cpython should be setup at
pub static CPYTHON: LazyLock<PathBuf> = LazyLock::new(|| REPO_DIR.join("cpython"));

/// Downloads the prepares the WASI-SDK for use in compilation steps
///
/// # Errors
/// Will error if WASI SDK cannot be downloaded, or if called on an unsupported OS or Architecture.
pub fn download_wasi_sdk() -> anyhow::Result<()> {
    const WASI_SDK_RELEASE: &str = "wasi-sdk-25";
    const WASI_SDK_VERSION: &str = "25.0";

    if !fs::exists(&*WASI_SDK)? {
        let arch = match env::consts::ARCH {
            arch @ "x86_64" => arch,
            "aarch64" => "arm64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture")),
        };
        let os @ ("linux" | "macos" | "windows") = env::consts::OS else {
            return Err(anyhow::anyhow!("Unsupported OS"));
        };

        let download_dir = format!("wasi-sdk-{WASI_SDK_VERSION}-{arch}-{os}");
        let bytes = reqwest::blocking::get(format!("https://github.com/WebAssembly/wasi-sdk/releases/download/{WASI_SDK_RELEASE}/{download_dir}.tar.gz"))?
        .error_for_status()?
        .bytes()?;

        Archive::new(GzDecoder::new(&bytes[..])).unpack(REPO_DIR.as_path())?;
        fs::rename(download_dir, &*WASI_SDK)?;

        // Hack for cpython to use wasip2 files. Uses wasip2 for wasi
        let sysroot_path = WASI_SDK.join("share").join("wasi-sysroot");
        for dir in ["include", "lib", "share"] {
            let dir = sysroot_path.join(dir);
            fs::rename(dir.join("wasm32-wasi"), dir.join("wasm32-wasi-bk"))?;
            fs::rename(dir.join("wasm32-wasip2"), dir.join("wasm32-wasi"))?;
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
pub fn download_and_compile_cpython() -> anyhow::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    const PYTHON_EXECUTABLE: &str = "python.exe";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    const PYTHON_EXECUTABLE: &str = "python";
    const GITHUB_USER: &str = "benbrandt";
    const GITHUB_REPO: &str = "cpython";
    const GITHUB_BRANCH: &str = "3.12-wasi";
    const PYTHON_VERSION: &str = "3.12";

    // Make sure WASI SDK is available
    download_wasi_sdk()?;

    if !fs::exists(&*CPYTHON)? {
        let bytes = reqwest::blocking::get(
            format!("https://github.com/{GITHUB_USER}/{GITHUB_REPO}/archive/refs/heads/{GITHUB_BRANCH}.tar.gz"),
        )?
        .error_for_status()?
        .bytes()?;

        Archive::new(GzDecoder::new(&bytes[..])).unpack(REPO_DIR.as_path())?;
        fs::rename(format!("{GITHUB_REPO}-{GITHUB_BRANCH}"), &*CPYTHON)?;
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
            fs::create_dir_all(&cpython_native_dir)?;
            fs::create_dir_all(&cpython_wasi_dir)?;

            run(Command::new("../../configure")
                .current_dir(&cpython_native_dir)
                .arg(format!(
                    "--prefix={}/install",
                    cpython_native_dir.to_str().unwrap()
                )))?;

            run(Command::new("make").current_dir(&cpython_native_dir))?;
        }

        let config_guess = String::from_utf8(run(
            Command::new("../../config.guess").current_dir(&cpython_wasi_dir)
        )?)?;

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
            ]))?;

        run(Command::new("make")
            .current_dir(&cpython_wasi_dir)
            .arg("install"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use bytes::Bytes;
    use flate2::bufread::GzDecoder;
    use heck::ToSnakeCase;
    use itertools::Itertools;
    use reqwest::Client;
    use serde::Deserialize;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use tar::Archive;
    use tempfile::tempdir;
    use tokio::fs;

    /// Registry information for a given Python project in the registry
    #[derive(Debug, Deserialize)]
    struct Project {
        name: String,
        files: Vec<ProjectFile>,
        versions: Vec<String>,
    }

    impl Project {
        /// Only return the relevant project file for the source code of each release
        fn sdist_files(self) -> HashMap<String, ProjectFile> {
            self.files
                .into_iter()
                .filter(|file| match file.yanked {
                    // Filter if yanked is true
                    Value::Bool(y) => !y,
                    // Or any value is present as a string
                    Value::String(_) => false,
                    _ => true,
                })
                .filter_map(|file| {
                    let filename = file.filename.rsplit_once(".tar.gz")?.0;
                    let version = filename
                        // Check for project name as is
                        .split_once(&format!("{}-", self.name))
                        .or_else(|| {
                            // Also check for snakecase version
                            filename.split_once(&format!("{}-", self.name.to_snake_case()))
                        })?
                        .1;
                    Some((version.to_owned(), file))
                })
                .collect()
        }
    }

    /// Information about a file that has been uploaded for a given project
    #[derive(Debug, Deserialize)]
    struct ProjectFile {
        filename: String,
        hashes: Hashes,
        url: String,
        yanked: Value,
    }

    impl ProjectFile {
        /// Download and validate the resulting file
        async fn download(&self) -> anyhow::Result<Bytes> {
            let bytes = Client::new()
                .get(&self.url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;

            if !self.hashes.valid(&bytes)? {
                return Err(anyhow::anyhow!("File doesn't match hash"));
            }

            Ok(bytes)
        }

        /// Download the sdist archive url and unpack it at the given destination
        async fn download_sdist_and_unpack(&self, dst: impl Into<PathBuf>) -> anyhow::Result<()> {
            if !self.filename.ends_with(".tar.gz") {
                return Err(anyhow::anyhow!(
                    "Project file should only be of sdist type and a gzipped tar archive."
                ));
            }

            let bytes = self.download().await?;
            let dst = dst.into();
            tokio::task::spawn_blocking(move || {
                Archive::new(GzDecoder::new(&bytes[..])).unpack(dst)
            })
            .await??;

            Ok(())
        }
    }

    #[derive(Debug, Deserialize)]
    struct Hashes {
        sha256: String,
    }

    impl Hashes {
        /// Whether or not the file is valid according to the hash
        fn valid(&self, bytes: impl AsRef<[u8]>) -> anyhow::Result<bool> {
            Ok(Sha256::digest(bytes)[..] == hex::decode(&self.sha256)?)
        }
    }

    /// A client for interacting with a PEP 691 compatible Simple Repository API
    struct PythonPackageIndex {
        client: Client,
        host: String,
    }

    impl PythonPackageIndex {
        fn new(host: impl Into<String>) -> Self {
            Self {
                client: Client::new(),
                host: host.into(),
            }
        }

        /// Returns all project information for a given package
        async fn project(&self, project_name: &str) -> anyhow::Result<Project> {
            Ok(self
                .client
                .get(format!("{}/simple/{project_name}/", &self.host))
                .header("Accept", "application/vnd.pypi.simple.v1+json")
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?)
        }
    }

    #[tokio::test]
    async fn can_retrieve_sdist_files_from_pypi() -> anyhow::Result<()> {
        let index = PythonPackageIndex::new("https://pypi.org");
        let project = index.project("pydantic-core").await?;

        let mut versions = project.versions.clone();
        versions.sort();
        let sdist_files = project.sdist_files();

        // There is one file for every version
        assert_eq!(versions.len(), sdist_files.len());
        // The keys match the versions
        assert_eq!(versions, sdist_files.into_keys().sorted().collect_vec());

        Ok(())
    }

    #[tokio::test]
    async fn can_download_specific_project_sdist_file() -> anyhow::Result<()> {
        let index = PythonPackageIndex::new("https://pypi.org");
        let sdist_files = index.project("pydantic-core").await?.sdist_files();
        let tempdir = tempdir()?;

        let file = sdist_files.get("2.27.1").unwrap();
        file.download_sdist_and_unpack(tempdir.path()).await?;

        let dir = fs::read_dir(tempdir.path())
            .await?
            .next_entry()
            .await?
            .unwrap();

        assert_eq!(dir.file_name(), "pydantic_core-2.27.1");
        assert!(dir.metadata().await?.is_dir());

        Ok(())
    }
}

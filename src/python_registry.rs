use std::{collections::HashMap, path::PathBuf};

use bytes::Bytes;
use flate2::bufread::GzDecoder;
use heck::ToSnakeCase;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tar::Archive;

use crate::REPO_DIR;

/// Download the sdist package for the specified project and version
///
/// # Errors
/// Will error if the project or version cannot be found or unpacked.
pub async fn download_sdist(
    project: &str,
    release_version: &str,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    PythonPackageIndex::default()
        .project(project)
        .await?
        .sdist(release_version)
        .ok_or(anyhow::anyhow!(
            "No version {release_version} for project {project}"
        ))?
        .download_sdist_and_unpack(output_dir.unwrap_or_else(|| REPO_DIR.join("sdist")))
        .await?;
    Ok(())
}

/// Registry information for a given Python project in the registry
#[derive(Debug, Deserialize)]
struct Project {
    /// Name of the project
    name: String,
    /// Files information available to download
    files: Vec<ProjectFile>,
    /// Available versions
    versions: Vec<String>,
}

impl Project {
    /// Only return the relevant project file for the source code of each release
    #[must_use]
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
                debug_assert!(self.versions.iter().any(|v| v == version));
                Some((version.to_owned(), file))
            })
            .collect()
    }

    /// Only return the sdist directory for the specified release
    #[must_use]
    fn sdist(self, version: &str) -> Option<ProjectFile> {
        self.sdist_files().remove(version)
    }
}

/// Information about a file that has been uploaded for a given project
#[derive(Debug, Deserialize)]
struct ProjectFile {
    /// Name of the file that can be downloaded
    filename: String,
    /// Hashes available for validating the file contents
    hashes: Hashes,
    /// Where the file is located
    url: String,
    /// Whether or not the file has been yanked
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
    ///
    /// # Errors
    /// Will error if the file fails to download or unpack at the given destination.
    async fn download_sdist_and_unpack(&self, dst: impl Into<PathBuf>) -> anyhow::Result<()> {
        if !self.filename.ends_with(".tar.gz") {
            return Err(anyhow::anyhow!(
                "Project file should only be of sdist type and a gzipped tar archive."
            ));
        }

        let bytes = self.download().await?;
        let dst = dst.into();
        tokio::task::spawn_blocking(move || Archive::new(GzDecoder::new(&bytes[..])).unpack(dst))
            .await??;

        Ok(())
    }
}

/// Hashes available for validating the file contents
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
#[derive(Debug)]
struct PythonPackageIndex {
    client: Client,
    host: String,
}

impl Default for PythonPackageIndex {
    fn default() -> Self {
        Self::new("https://pypi.org")
    }
}

impl PythonPackageIndex {
    /// Generate new client that points to the given host
    fn new(host: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            host: host.into(),
        }
    }

    /// Returns all project information for a given package
    ///
    /// # Errors
    /// Will error if host does not support JSON version of registry information
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

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use tempfile::tempdir;
    use tokio::fs;

    use super::*;

    #[tokio::test]
    async fn can_retrieve_sdist_files_from_pypi() -> anyhow::Result<()> {
        let index = PythonPackageIndex::default();
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
        let tempdir = tempdir()?;
        let project = PythonPackageIndex::default()
            .project("pydantic-core")
            .await?;

        let file = project.sdist("2.27.1").unwrap();
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

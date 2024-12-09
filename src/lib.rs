//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bytes::Bytes;
    use heck::ToSnakeCase;
    use itertools::Itertools;
    use reqwest::Client;
    use serde::Deserialize;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
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
        let bytes = file.download().await?;

        fs::write(tempdir.path().join(&file.filename), bytes).await?;

        Ok(())
    }
}

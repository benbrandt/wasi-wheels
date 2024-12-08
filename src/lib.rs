//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use reqwest::Client;
    use serde::Deserialize;

    /// Registry information for a given Python project in the registry
    #[derive(Deserialize)]
    struct Project {
        files: Vec<ProjectFile>,
        versions: Vec<String>,
    }

    impl Project {
        fn sdist_files(self) -> HashMap<String, ProjectFile> {
            self.files
                .into_iter()
                .filter(|file| file.filename.ends_with("tar.gz"))
                .map(|file| (file.filename.clone(), file))
                .collect()
        }
    }

    /// Information about a file that has been uploaded for a given project
    #[derive(Deserialize)]
    struct ProjectFile {
        filename: String,
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
                .json()
                .await?)
        }
    }

    #[tokio::test]
    async fn can_retrieve_sdist_files_from_pypi() -> anyhow::Result<()> {
        let index = PythonPackageIndex::new("https://pypi.org");
        let project_name = "pydantic-core";
        let project = index.project(project_name).await?;

        assert_eq!(project.versions.len(), project.sdist_files().len());

        Ok(())
    }
}

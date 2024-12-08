//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use heck::ToSnakeCase;
    use itertools::Itertools;
    use reqwest::Client;
    use serde::Deserialize;

    /// Registry information for a given Python project in the registry
    #[derive(Deserialize)]
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
        let mut versions = project.versions.clone();
        versions.sort();
        let sdist_files = project.sdist_files();

        // There is one file for every version
        assert_eq!(versions.len(), sdist_files.len());
        // The keys match the versions
        assert_eq!(versions, sdist_files.into_keys().sorted().collect_vec());

        Ok(())
    }
}

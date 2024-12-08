//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

#[cfg(test)]
mod tests {
    use reqwest::Client;
    use serde::Deserialize;

    /// Registry information for a given Python project in the registry
    #[derive(Deserialize)]
    struct Project {
        #[expect(dead_code)]
        files: Vec<ProjectFile>,
    }

    /// Information about a file that has been uploaded for a given project
    #[derive(Deserialize)]
    struct ProjectFile {
        #[expect(dead_code)]
        filename: String,
    }

    /// A client for interacting with a PEP 691 compatible Simple Repository API
    struct PythonPackageIndex {
        client: Client,
    }

    impl PythonPackageIndex {
        fn new() -> Self {
            Self {
                client: Client::new(),
            }
        }

        async fn project(&self, package_name: &str) -> anyhow::Result<Project> {
            Ok(self
                .client
                .get(format!("https://pypi.org/simple/{package_name}/"))
                .header("Accept", "application/vnd.pypi.simple.v1+json")
                .send()
                .await?
                .json()
                .await?)
        }
    }

    #[tokio::test]
    async fn can_parse_pydantic_info_from_pypi() -> anyhow::Result<()> {
        let _resp = PythonPackageIndex::new().project("pydantic-core").await?;

        Ok(())
    }
}

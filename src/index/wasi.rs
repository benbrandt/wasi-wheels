//! Generate a custom index for WASI wheels.

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    };

    use futures_util::TryStreamExt;
    use octocrab::Octocrab;
    use rinja::Template;
    use tokio::pin;

    struct Packages {
        packages: HashSet<String>,
    }

    impl Packages {
        fn new() -> Self {
            Self {
                packages: HashSet::new(),
            }
        }

        fn insert(&mut self, package: String) {
            self.packages.insert(package);
        }

        /// Generate the root index template for all packages
        fn generate_index(&self) -> anyhow::Result<String> {
            #[derive(Template)]
            #[template(path = "index.html")]
            struct IndexTemplate {
                packages: HashSet<String>,
            }

            Ok(IndexTemplate {
                packages: self.packages.clone(),
            }
            .render()?)
        }

        /// Returns hashmap of key Package Name and value rendered template
        fn generate_package_files(&self) -> anyhow::Result<HashMap<String, String>> {
            #[derive(Template)]
            #[template(path = "package_files.html")]
            struct PackageFilesTemplate {
                package: String,
                files: HashSet<WheelFile>,
            }

            let mut templates = HashMap::new();

            for package in &self.packages {
                templates.insert(
                    package.clone(),
                    PackageFilesTemplate {
                        package: package.clone(),
                        files: HashSet::new(),
                    }
                    .render()?,
                );
            }

            Ok(templates)
        }
    }

    /// A file in the index for a given package
    struct WheelFile {
        /// URL that can be used to download the wheel
        url: String,
        /// The filename to render
        name: String,
    }

    /// GitHub client for loading release information from a repository.
    struct GitHubReleaseClient {
        client: Arc<Octocrab>,
    }

    impl GitHubReleaseClient {
        /// Creates a new instance of a GitHub Client initialized with the default Octocrab instance.
        fn new() -> Self {
            Self {
                client: octocrab::instance(),
            }
        }

        /// Retrieves a set of package names from a GitHub repository's releases.
        /// Assumes the release tags follow the structure <package-name>/v<package-version>.
        ///
        /// # Arguments
        /// * `owner` - The owner of the GitHub repository
        /// * `repo` - The name of the GitHub repository
        ///
        /// # Returns
        /// A `Result` containing a `HashSet` of package names found in release tags.
        async fn packages(&self, owner: &str, repo: &str) -> anyhow::Result<Packages> {
            let releases = self
                .client
                .repos(owner, repo)
                .releases()
                .list()
                .send()
                .await?
                .into_stream(&self.client);
            pin!(releases);

            let mut packages = Packages::new();

            while let Some(release) = releases.try_next().await? {
                let Some((package, _)) = release.tag_name.split_once("/v") else {
                    continue;
                };
                packages.insert(package.to_owned());
            }

            Ok(packages)
        }
    }

    #[tokio::test]
    async fn generate_package_index() -> anyhow::Result<()> {
        let releases = GitHubReleaseClient::new();
        let packages = releases.packages("benbrandt", "wasi-wheels").await?;

        let index = packages.generate_index()?;

        assert!(packages
            .packages
            .iter()
            .all(|package| index.contains(&format!("<a href=\"/{package}/\">{package}</a>"))));

        Ok(())
    }

    #[tokio::test]
    async fn generate_package_files() -> anyhow::Result<()> {
        let releases = GitHubReleaseClient::new();
        let packages = releases.packages("benbrandt", "wasi-wheels").await?;

        let templates = packages.generate_package_files()?;

        assert_eq!(packages.packages.len(), templates.len());

        // for template in templates.values() {
        // assert!(template.contains("<a href"));
        // assert!(template.contains("#sha256="));
        // }

        Ok(())
    }
}

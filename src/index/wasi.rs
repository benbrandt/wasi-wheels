//! Generate a custom index for WASI wheels.

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use futures_util::TryStreamExt;
    use octocrab::Octocrab;
    use rinja::Template;
    use tokio::pin;

    #[derive(Template)]
    #[template(path = "index.html")]
    struct IndexTemplate {
        packages: HashSet<String>,
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
        async fn packages(&self, owner: &str, repo: &str) -> anyhow::Result<HashSet<String>> {
            let releases = self
                .client
                .repos(owner, repo)
                .releases()
                .list()
                .send()
                .await?
                .into_stream(&self.client);
            pin!(releases);

            let mut packages = HashSet::new();

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
    async fn get_releases() -> anyhow::Result<()> {
        let releases = GitHubReleaseClient::new();
        let packages = releases.packages("benbrandt", "wasi-wheels").await?;

        let index = IndexTemplate {
            packages: packages.clone(),
        }
        .render()?;

        assert!(packages
            .iter()
            .all(|package| index.contains(&format!("<a href=\"/{package}/\">{package}</a>"))));

        Ok(())
    }
}

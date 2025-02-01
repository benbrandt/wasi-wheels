//! Generate a custom index for WASI wheels.
use std::{collections::HashMap, path::Path, sync::Arc};

use futures_util::TryStreamExt;
use itertools::Itertools;
use octocrab::{models::repos::Asset, Octocrab};
use reqwest::Client;
use rinja::Template;
use tokio::{fs, pin};
use url::Url;

pub struct Packages {
    http: Client,
    packages: HashMap<String, HashMap<String, WheelFile>>,
}

impl Packages {
    fn new() -> Self {
        Self {
            http: Client::builder().use_rustls_tls().build().unwrap(),
            packages: HashMap::new(),
        }
    }

    async fn extend(&mut self, package: &str, mut assets: Vec<Asset>) -> anyhow::Result<()> {
        // Pull hashes file out first so we can add them after
        let hashes = assets
            .iter()
            .position(|a| a.name == "hashes.txt")
            .map(|index| assets.remove(index));

        // Process wheel files
        let packages = self.packages.entry(package.to_owned()).or_default();
        for asset in assets.into_iter().filter(|a| {
            Path::new(&a.name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("whl"))
        }) {
            packages.insert(
                asset.name.clone(),
                WheelFile {
                    url: asset.browser_download_url,
                    name: asset.name,
                },
            );
        }

        // Add in the hashes
        if let Some(hashes) = hashes {
            let txt = self
                .http
                .get(hashes.browser_download_url)
                .send()
                .await?
                .text()
                .await?;
            for (file, hash) in txt.lines().filter_map(|line| line.split_once('\t')) {
                let file = packages.get_mut(file).expect("File should already exist.");
                file.url.set_fragment(Some(&format!("sha256={hash}")));
            }
        }

        Ok(())
    }

    /// Output the index to a given path
    pub async fn generate_index(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir(path).await?;
        }
        fs::write(path.join("index.html"), self.render_index()?).await?;
        for (package, template) in self.render_package_files()? {
            let dir = path.join(package);
            if !dir.exists() {
                fs::create_dir(&dir).await?;
            }
            fs::write(dir.join("index.html"), template).await?;
        }
        Ok(())
    }

    /// Generate the root index template for all packages
    fn render_index(&self) -> anyhow::Result<String> {
        #[derive(Template)]
        #[template(path = "index.html")]
        struct IndexTemplate<'a> {
            packages: Vec<&'a str>,
        }

        Ok(IndexTemplate {
            packages: self.packages.keys().map(String::as_str).sorted().collect(),
        }
        .render()?)
    }

    /// Returns hashmap of key Package Name and value rendered template
    fn render_package_files(&self) -> anyhow::Result<HashMap<String, String>> {
        #[derive(Template)]
        #[template(path = "package_files.html")]
        struct PackageFilesTemplate<'a> {
            package: &'a str,
            files: Vec<&'a WheelFile>,
        }

        let mut templates = HashMap::new();

        for (package, files) in &self.packages {
            templates.insert(
                package.clone(),
                PackageFilesTemplate {
                    package,
                    files: files.values().sorted().collect(),
                }
                .render()?,
            );
        }

        Ok(templates)
    }
}

/// A file in the index for a given package
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct WheelFile {
    /// URL that can be used to download the wheel
    url: Url,
    /// The filename to render
    name: String,
}

/// GitHub client for loading release information from a repository.
pub struct GitHubReleaseClient {
    client: Arc<Octocrab>,
}

impl GitHubReleaseClient {
    /// Creates a new instance of a GitHub Client initialized with the default Octocrab instance.
    pub fn new() -> Self {
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
    pub async fn packages(&self, owner: &str, repo: &str) -> anyhow::Result<Packages> {
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
            packages.extend(package, release.assets).await?;
        }

        Ok(packages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn generate_package_files() -> anyhow::Result<()> {
        let releases = GitHubReleaseClient::new();
        let packages = releases.packages("benbrandt", "wasi-wheels").await?;
        let temp_dir = tempfile::tempdir()?;
        let dir = temp_dir.path();

        packages.generate_index(dir).await?;

        let index = fs::read_to_string(dir.join("index.html")).await?;

        assert!(packages
            .packages
            .keys()
            .all(|package| index.contains(&format!("<a href=\"{package}/\">{package}</a>"))));

        // Test individual packages
        for (package, files) in packages.packages {
            assert!(!files.is_empty());
            let template = fs::read_to_string(dir.join(package).join("index.html")).await?;
            for file in files.values() {
                assert!(template.contains(&format!("<a href=\"{}\">{}</a>", file.url, file.name)));
                assert!(file.url.as_str().contains("#sha256="));
            }
        }

        Ok(())
    }
}

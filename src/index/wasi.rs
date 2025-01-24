//! Generate a custom index for WASI wheels.

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use futures_util::TryStreamExt;
    use rinja::Template;
    use tokio::pin;

    #[derive(Template)]
    #[template(path = "index.html")]
    struct IndexTemplate {
        packages: HashSet<String>,
    }

    #[tokio::test]
    async fn get_releases() -> anyhow::Result<()> {
        let github = octocrab::instance();
        let releases = github
            .repos("benbrandt", "wasi-wheels")
            .releases()
            .list()
            .send()
            .await?
            .into_stream(&github);
        pin!(releases);

        let mut packages = HashSet::new();

        while let Some(release) = releases.try_next().await? {
            let Some((package, _)) = release.tag_name.split_once("/v") else {
                continue;
            };
            packages.insert(package.to_owned());
        }

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

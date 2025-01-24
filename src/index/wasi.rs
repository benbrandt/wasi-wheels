//! Generate a custom index for WASI wheels.

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn get_releases() -> anyhow::Result<()> {
        let github = octocrab::instance();
        let releases = github
            .repos("benbrandt", "wasi-wheels")
            .releases()
            .list()
            .per_page(1)
            .send()
            .await?;

        for release in releases {
            dbg!(release);
        }

        Ok(())
    }
}

use ebg::{
    generator::{self, generate_site},
    site::Site,
};
use eyre::Context;
use tracing::info;

impl super::Command for generator::Options {
    async fn run(self) -> eyre::Result<()> {
        info!("building blog from {}", self.path.display());

        let site = Site::from_directory(&self.path, self.unpublished)
            .await
            .context("loading site content")?;
        generate_site(&site, &self)
            .await
            .context("generating site")?;

        Ok(())
    }
}

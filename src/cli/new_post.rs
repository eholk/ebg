use clap::Parser;
use tokio::io::AsyncWriteExt;
use tracing::debug;

#[derive(Parser)]
pub struct NewPostOptions {
    title: String,
}

impl super::Command for NewPostOptions {
    async fn run(self) -> eyre::Result<()> {
        // make sure there's a Site.toml in the current directory
        let root = std::env::current_dir()?;
        let site_toml = root.join("Site.toml");
        if !site_toml.exists() {
            return Err(eyre::eyre!(
                "No Site.toml found in current directory: {}",
                root.display()
            ));
        }

        let posts_dir = root.join("_posts");

        if !posts_dir.exists() {
            tokio::fs::create_dir(&posts_dir).await?;
        }

        let post_filename = posts_dir.join(format!(
            "{}-{}.md",
            chrono::Local::now().format("%Y-%m-%d"),
            slug::slugify(&self.title)
        ));
        debug!("creating new post at {}", post_filename.display());

        let mut file = tokio::fs::File::create(&post_filename).await?;
        file.write_all(
            format!(
                r#"---
layout: post
published: false
---

# {title}
"#,
                title = self.title
            )
            .as_bytes(),
        )
        .await?;

        Ok(())
    }
}
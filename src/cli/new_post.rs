use std::{
    fs::{create_dir, File},
    io::Write,
};

use clap::Parser;
use miette::IntoDiagnostic;
use tracing::debug;

#[derive(Parser)]
pub struct NewPostOptions {
    title: String,
    #[clap(long)]
    open: bool,
}

impl super::Command for NewPostOptions {
    fn run(self) -> miette::Result<()> {
        // make sure there's a Site.toml in the current directory
        let root = std::env::current_dir().unwrap();
        let site_toml = root.join("Site.toml");
        if !site_toml.exists() {
            return Err(miette::miette!(
                "No Site.toml found in current directory: {}",
                root.display()
            ));
        }

        let posts_dir = root.join("_posts");

        if !posts_dir.exists() {
            create_dir(&posts_dir).into_diagnostic()?;
        }

        let post_filename = posts_dir.join(format!(
            "{}-{}.md",
            chrono::Local::now().format("%Y-%m-%d"),
            slug::slugify(&self.title)
        ));
        debug!("creating new post at {}", post_filename.display());

        let mut file = File::create(&post_filename).into_diagnostic()?;
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
        .into_diagnostic()?;

        if self.open {
            open::that_detached(post_filename).into_diagnostic()?;
        }

        Ok(())
    }
}

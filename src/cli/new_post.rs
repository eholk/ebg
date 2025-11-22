use std::{
    fs::{File, create_dir},
    io::Write,
};

use chrono::NaiveDate;
use clap::Parser;
use miette::IntoDiagnostic;
use tracing::debug;

#[derive(Parser)]
pub struct NewPostOptions {
    title: String,
    /// Open the new post in the default editor
    #[clap(long)]
    open: bool,

    /// Set the publish date for the new post
    #[clap(long, default_value_t = chrono::Local::now().date_naive())]
    date: NaiveDate,
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
            self.date.format("%Y-%m-%d"),
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

#[cfg(test)]
mod test {
    use chrono::NaiveDate;
    use clap::Parser;

    use super::NewPostOptions;

    #[test]
    fn parse_new_post_date() {
        let args = shlex::split("ebg \"Hello, World\" --date 2025-11-21").unwrap();
        let cmd = NewPostOptions::parse_from(args);

        assert_eq!(cmd.title, "Hello, World");
        assert_eq!(cmd.date, NaiveDate::from_ymd_opt(2025, 11, 21).unwrap());
    }
}

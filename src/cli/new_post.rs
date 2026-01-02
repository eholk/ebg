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
    ///
    /// Defaults to today.
    #[clap(long)]
    date: Option<NaiveDate>,

    /// Create the post as a directory with an index.md file
    #[clap(long)]
    dir: bool,
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

        let date_slug = format!(
            "{}-{}",
            self.date
                .unwrap_or_else(|| chrono::Local::now().date_naive())
                .format("%Y-%m-%d"),
            slug::slugify(&self.title)
        );

        let post_filename = if self.dir {
            let post_dir = posts_dir.join(&date_slug);
            create_dir(&post_dir).into_diagnostic()?;
            post_dir.join("index.md")
        } else {
            posts_dir.join(format!("{}.md", date_slug))
        };

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
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    use super::super::Command;
    use super::NewPostOptions;

    // Mutex to ensure tests that change current directory don't run in parallel
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_new_post_date() {
        let args = shlex::split("ebg \"Hello, World\" --date 2025-11-21").unwrap();
        let cmd = NewPostOptions::parse_from(args);

        assert_eq!(cmd.title, "Hello, World");
        assert_eq!(cmd.date, NaiveDate::from_ymd_opt(2025, 11, 21));
        assert!(!cmd.dir);
    }

    #[test]
    fn parse_new_post_with_dir_flag() {
        let args = shlex::split("ebg \"Hello, World\" --dir").unwrap();
        let cmd = NewPostOptions::parse_from(args);

        assert_eq!(cmd.title, "Hello, World");
        assert!(cmd.dir);
    }

    #[test]
    fn parse_new_post_with_dir_and_date() {
        let args = shlex::split("ebg \"Hello, World\" --dir --date 2025-11-21").unwrap();
        let cmd = NewPostOptions::parse_from(args);

        assert_eq!(cmd.title, "Hello, World");
        assert_eq!(cmd.date, NaiveDate::from_ymd_opt(2025, 11, 21));
        assert!(cmd.dir);
    }

    #[test]
    fn create_regular_post_file() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Use a closure to ensure we always restore the directory
        let result = (|| {
            // Change to temp directory
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create Site.toml
            fs::write("Site.toml", "title = \"Test Site\"\n").unwrap();

            // Create a new post
            let cmd = NewPostOptions {
                title: "Test Post".to_string(),
                open: false,
                date: Some(NaiveDate::from_ymd_opt(2025, 11, 22).unwrap()),
                dir: false,
            };

            cmd.run().unwrap();

            // Check that the post file was created
            let post_path = temp_dir.path().join("_posts/2025-11-22-test-post.md");
            assert!(post_path.exists());

            // Check the content
            let content = fs::read_to_string(&post_path).unwrap();
            assert!(content.contains("# Test Post"));
            assert!(content.contains("layout: post"));
            assert!(content.contains("published: false"));
        })();

        // Always restore original directory, even if test panics
        std::env::set_current_dir(original_dir).unwrap();
        result
    }

    #[test]
    fn create_directory_based_post() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Use a closure to ensure we always restore the directory
        let result = (|| {
            // Change to temp directory
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create Site.toml
            fs::write("Site.toml", "title = \"Test Site\"\n").unwrap();

            // Create a new post with directory structure
            let cmd = NewPostOptions {
                title: "Test Directory Post".to_string(),
                open: false,
                date: Some(NaiveDate::from_ymd_opt(2025, 11, 22).unwrap()),
                dir: true,
            };

            cmd.run().unwrap();

            // Check that the directory was created
            let post_dir = temp_dir
                .path()
                .join("_posts/2025-11-22-test-directory-post");
            assert!(post_dir.exists());
            assert!(post_dir.is_dir());

            // Check that index.md was created
            let index_path = post_dir.join("index.md");
            assert!(index_path.exists());

            // Check the content
            let content = fs::read_to_string(&index_path).unwrap();
            assert!(content.contains("# Test Directory Post"));
            assert!(content.contains("layout: post"));
            assert!(content.contains("published: false"));
        })();

        // Always restore original directory, even if test panics
        std::env::set_current_dir(original_dir).unwrap();
        result
    }
}

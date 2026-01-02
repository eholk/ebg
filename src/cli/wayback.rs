use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use clap::{Parser, Subcommand};
use ebg::index::{SiteIndex, WaybackFilter, WaybackLink, WaybackLinks};
use ebg::wayback::Wayback;
use miette::{Context, IntoDiagnostic};
use url::Url;

use super::Command;

#[derive(Parser)]
pub struct WaybackOptions {
    #[clap(subcommand)]
    command: WaybackCommands,
}

#[derive(Subcommand)]
enum WaybackCommands {
    /// Scan posts and update wayback archive links
    UpdateLinks(UpdateLinksOptions),
}

#[derive(Parser)]
struct UpdateLinksOptions {
    /// The root directory of the site (defaults to current directory)
    #[clap(default_value = ".")]
    root: PathBuf,

    /// Only show what would be archived without actually archiving
    #[clap(long)]
    dry_run: bool,

    /// Delay in seconds between archiving requests (default: 1)
    #[clap(long, default_value = "1")]
    delay: u64,
}

impl Command for WaybackOptions {
    fn run(self) -> miette::Result<()> {
        match self.command {
            WaybackCommands::UpdateLinks(options) => options.run(),
        }
    }
}

impl Command for UpdateLinksOptions {
    fn run(self) -> miette::Result<()> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move { self.run_async().await })
    }
}

impl UpdateLinksOptions {
    async fn run_async(self) -> miette::Result<()> {
        println!("Loading site from {}...", self.root.display());

        // Load the site index
        let site = SiteIndex::from_directory(&self.root, true).await?;

        // Get wayback API credentials if not in dry-run mode
        let wayback_client = if !self.dry_run {
            let access_key = std::env::var("WAYBACK_ACCESS_KEY")
                .into_diagnostic()
                .wrap_err("WAYBACK_ACCESS_KEY environment variable not set")?;
            let secret_key = std::env::var("WAYBACK_SECRET_KEY")
                .into_diagnostic()
                .wrap_err("WAYBACK_SECRET_KEY environment variable not set")?;
            Some(Wayback::with_credentials(access_key, secret_key))
        } else {
            println!("🔍 DRY RUN MODE - No links will be archived\n");
            None
        };

        // Show active filters
        if let Some(wayback_cfg) = site.config().wayback.as_ref() {
            if !wayback_cfg.exclude.is_empty() {
                println!("📋 Active filters:");
                for filter in &wayback_cfg.exclude {
                    match filter {
                        WaybackFilter::Before(date) => {
                            println!("   • Excluding posts before {}", date.format("%Y-%m-%d"));
                        }
                    }
                }
                println!();
            }
        }

        let mut total_posts = 0;
        let mut total_links = 0;
        let mut already_archived = 0;
        let mut newly_archived = 0;
        let mut failed_archives = 0;
        let mut filtered_posts = 0;

        // Get wayback config for filtering
        let wayback_config = site.config().wayback.as_ref();

        // Iterate through all posts
        for post in site.posts() {
            total_posts += 1;

            // Check if post should be excluded by filters
            if let Some(config) = wayback_config {
                if config.should_exclude_post(post) {
                    filtered_posts += 1;
                    continue;
                }
            }

            // Extract external links from the post
            let external_links: Vec<_> = post.external_links().collect();

            if external_links.is_empty() {
                continue;
            }

            total_links += external_links.len();

            // Determine the wayback config file path
            let source_path = post.source_path();
            let wayback_path = if source_path.ends_with("index.md") {
                // Directory-based post: _posts/2023-01-25-hello/index.md -> _posts/2023-01-25-hello/wayback.toml
                source_path.parent().unwrap().join("wayback.toml")
            } else {
                // Single-file post: _posts/2023-01-25-hello.md -> _posts/2023-01-25-hello.wayback.toml
                source_path.with_extension("wayback.toml")
            };

            let wayback_file_path = self.root.join(&wayback_path);

            // Load existing wayback links if they exist
            let mut wayback_links = if wayback_file_path.exists() {
                WaybackLinks::from_file(&wayback_file_path)?
            } else {
                WaybackLinks::new()
            };

            // Check which links need archiving
            let mut post_needs_archiving = Vec::new();
            let mut post_already_archived = Vec::new();

            for url in external_links {
                if wayback_links.contains(&url) {
                    post_already_archived.push(url);
                } else {
                    post_needs_archiving.push(url);
                }
            }

            already_archived += post_already_archived.len();

            // Process links that need archiving
            if !post_needs_archiving.is_empty() {
                println!("\n📄 {}", source_path.display());
                println!("   Wayback config: {}", wayback_path.display());
                println!("   ✅ Already archived: {}", post_already_archived.len());
                println!("   📦 Needs archiving: {}", post_needs_archiving.len());

                if let Some(client) = &wayback_client {
                    // Actually archive the links
                    for (idx, url) in post_needs_archiving.iter().enumerate() {
                        println!(
                            "   [{}/{}] Archiving {}...",
                            idx + 1,
                            post_needs_archiving.len(),
                            url
                        );

                        match self.archive_link(client, url).await {
                            Ok(wayback_link) => {
                                println!("      ✅ Archived: {}", wayback_link.wayback_url);
                                wayback_links.add(wayback_link);
                                newly_archived += 1;

                                // Save progress incrementally
                                wayback_links
                                    .to_file(&wayback_file_path)
                                    .wrap_err_with(|| {
                                        format!("Failed to save wayback config to {}", wayback_path.display())
                                    })?;
                            }
                            Err(e) => {
                                println!("      ❌ Failed: {}", e);
                                failed_archives += 1;
                            }
                        }

                        // Add delay between requests to be respectful
                        if idx < post_needs_archiving.len() - 1 {
                            tokio::time::sleep(Duration::from_secs(self.delay)).await;
                        }
                    }
                } else {
                    // Dry run - just list what would be archived
                    for url in &post_needs_archiving {
                        println!("      - {}", url);
                    }
                }
            }
        }

        // Print overall summary
        println!("\n{}", "=".repeat(60));
        println!("Summary:");
        println!("  Posts scanned: {}", total_posts);
        if filtered_posts > 0 {
            println!("  🚫 Filtered by config: {}", filtered_posts);
        }
        println!("  Total external links: {}", total_links);
        println!("  ✅ Already archived: {}", already_archived);

        if self.dry_run {
            println!("  📦 Would archive: {}", total_links - already_archived);
            println!("\n⚠️  This was a dry run. Use without --dry-run to actually archive links.");
        } else {
            println!("  ✨ Newly archived: {}", newly_archived);
            if failed_archives > 0 {
                println!("  ❌ Failed: {}", failed_archives);
            }
        }

        Ok(())
    }

    /// Archives a single link and returns the WaybackLink on success.
    async fn archive_link(&self, client: &Wayback, url: &Url) -> miette::Result<WaybackLink> {
        // Start the archive job
        let job = client
            .begin_save_page(url)
            .await
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to start archiving {}", url))?;

        // Poll for completion
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            let status = client
                .job_status(&job)
                .await
                .into_diagnostic()
                .wrap_err("Failed to check job status")?;

            if !status.is_complete() {
                continue;
            }

            if !status.is_success() {
                return Err(miette::miette!(
                    "Archive job failed with status: {}",
                    status.status()
                ));
            }

            // Extract wayback URL
            let wayback_url = status
                .wayback_url()
                .ok_or_else(|| miette::miette!("No wayback URL in successful response"))?;

            // Get timestamp for the archived_at field
            let archived_at = Utc::now();

            return Ok(WaybackLink {
                url: url.clone(),
                wayback_url,
                archived_at,
            });
        }
    }
}

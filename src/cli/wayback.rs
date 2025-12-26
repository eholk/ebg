use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ebg::index::{SiteIndex, WaybackFilter, WaybackLinks};

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
            .block_on(async move {
                println!("Loading site from {}...", self.root.display());

                // Load the site index
                let site = SiteIndex::from_directory(&self.root, true).await?;

                // Show active filters
                if let Some(wayback_cfg) = site.config().wayback.as_ref() {
                    if !wayback_cfg.exclude.is_empty() {
                        println!("\n📋 Active filters:");
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
                let mut needs_archiving = 0;
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

                    // Load existing wayback links if they exist
                    let wayback_config = if self.root.join(&wayback_path).exists() {
                        WaybackLinks::from_file(self.root.join(&wayback_path))?
                    } else {
                        WaybackLinks::new()
                    };

                    // Check which links need archiving
                    let mut post_needs_archiving = Vec::new();
                    let mut post_already_archived = Vec::new();

                    for url in external_links {
                        if wayback_config.contains(&url) {
                            post_already_archived.push(url);
                        } else {
                            post_needs_archiving.push(url);
                        }
                    }

                    already_archived += post_already_archived.len();
                    needs_archiving += post_needs_archiving.len();

                    // Print summary for this post if it has links needing archiving
                    if !post_needs_archiving.is_empty() {
                        println!("\n📄 {}", source_path.display());
                        println!("   Wayback config: {}", wayback_path.display());
                        println!("   ✅ Already archived: {}", post_already_archived.len());
                        println!("   📦 Needs archiving: {}", post_needs_archiving.len());

                        for url in &post_needs_archiving {
                            println!("      - {}", url);
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
                println!("  📦 Need archiving: {}", needs_archiving);

                if needs_archiving > 0 {
                    println!("\n⚠️  This is a dry run. No links were archived.");
                    println!("   (API integration coming in Phase 4)");
                }

                Ok(())
            })
    }
}

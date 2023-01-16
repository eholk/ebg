use std::time::Instant;

use clap::Parser;
use eyre::WrapErr;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};
use tracing::info;

use ebg::{command_line::Options, generate::generate_site, site::Site};

#[derive(Parser)]
enum Cli {
    Build(Options),
    About,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let start = Instant::now();
    let args = Cli::parse();

    tracing_subscriber::fmt()
        .pretty()
        // .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .init();

    match args {
        Cli::Build(args) => {
            info!("building blog from {}", args.path.display());

            let site = Site::from_directory(&args.path, args.unpublished)
                .await
                .context("loading site content")?;
            generate_site(&site, &args)
                .await
                .context("generating site")?;

            info!(
                "Generating site took {:.3} seconds",
                start.elapsed().as_secs_f32()
            );
        }
        Cli::About => {
            println!("# Syntax Highlighting #");
            println!();
            println!("## Languages ##");
            println!();
            let ss = SyntaxSet::load_defaults_newlines();
            for (i, lang) in ss.syntaxes().iter().enumerate() {
                println!(
                    "{}: {} ({})",
                    i + 1,
                    lang.name,
                    lang.file_extensions.join(", ")
                );
            }
            println!();
            println!();
            println!("## Themes ##");
            println!();
            let ts = ThemeSet::load_defaults();
            for (i, theme) in ts.themes.keys().enumerate() {
                println!("{}: {theme}", i + 1);
            }
            println!();
        }
    }

    Ok(())
}

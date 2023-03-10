use std::time::Instant;

use clap::Parser;
use eyre::WrapErr;
use serve::ServerOptions;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};
use tracing::info;

use ebg::{
    generator::{generate_site, Options},
    site::Site,
};
use tracing_subscriber::{prelude::*, EnvFilter};

mod serve;

#[derive(Parser)]
enum Cli {
    Build(Options),
    Serve(ServerOptions),
    About,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let start = Instant::now();
    let args = Cli::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(EnvFilter::from_env("EBG_LOG"))
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
        Cli::Serve(options) => serve::serve(options).await?,
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

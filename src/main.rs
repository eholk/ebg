#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use clap::Parser;
use cli::{about::AboutOptions, new_post::NewPostOptions};
use serve::ServerOptions;

use ebg::generator::Options;
use tracing_subscriber::{prelude::*, EnvFilter};

use crate::cli::Command;

mod cli;
mod serve;

#[derive(Parser)]
enum Cli {
    Build(Options),
    Serve(ServerOptions),
    NewPost(NewPostOptions),
    About(AboutOptions),
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Cli::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(EnvFilter::from_env("EBG_LOG"))
        .init();

    match args {
        Cli::Build(args) => args.run().await?,
        Cli::NewPost(options) => options.run().await?,
        Cli::Serve(options) => options.run().await?,
        Cli::About(cmd) => cmd.run().await?,
    }

    Ok(())
}

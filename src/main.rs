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

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(EnvFilter::from_env("EBG_LOG"))
        .init();

    match args {
        Cli::Build(args) => args.run()?,
        Cli::NewPost(options) => options.run()?,
        Cli::Serve(options) => options.run()?,
        Cli::About(cmd) => cmd.run()?,
    }

    Ok(())
}

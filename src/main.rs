use clap::Parser;
use cli::{about::AboutOptions, new_post::NewPostOptions};
use serve::ServerOptions;

use ebg::generator::Options;
use tracing_subscriber::{prelude::*, EnvFilter};

use crate::cli::Command;

mod cli;
mod serve;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Parser)]
enum Commands {
    Build(Options),
    Serve(ServerOptions),
    NewPost(NewPostOptions),
    About(AboutOptions),
}

fn main() -> miette::Result<()> {
    let args = Cli::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(EnvFilter::from_env("EBG_LOG"))
        .init();

    match args.command {
        Commands::Build(args) => args.run()?,
        Commands::NewPost(options) => options.run()?,
        Commands::Serve(options) => options.run()?,
        Commands::About(cmd) => cmd.run()?,
    }

    Ok(())
}

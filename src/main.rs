use clap::Parser;
use cli::{
    about::AboutOptions, list::ListOptions, new_post::NewPostOptions, wayback::WaybackOptions,
};
use serve::ServerOptions;

use ebg::generator::Options;
use tracing_subscriber::{EnvFilter, prelude::*};

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
    About(AboutOptions),
    Build(Options),
    List(ListOptions),
    NewPost(NewPostOptions),
    Serve(ServerOptions),
    Wayback(WaybackOptions),
}

fn main() -> miette::Result<()> {
    let args = Cli::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(EnvFilter::from_env("EBG_LOG"))
        .init();

    match args.command {
        Commands::About(cmd) => cmd.run()?,
        Commands::Build(args) => args.run()?,
        Commands::List(args) => args.run()?,
        Commands::NewPost(options) => options.run()?,
        Commands::Serve(options) => options.run()?,
        Commands::Wayback(options) => options.run()?,
    }

    Ok(())
}

use clap::Parser;
use cli::{about::AboutOptions, new_post::NewPostOptions};
use serve::ServerOptions;

use ebg::generator::Options;
use tracing_subscriber::{prelude::*, EnvFilter};

use crate::cli::Command;

mod cli;
mod serve;

#[derive(Parser)]
struct Cli {
    /// Print the version number
    #[clap(long)]
    version: bool,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser)]
enum Commands {
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

    if args.version {
        println!("ebg {}", env!("CARGO_PKG_VERSION"));
        if args.command.is_none() {
            return Ok(());
        }
    }

    match args.command {
        Some(Commands::Build(args)) => args.run()?,
        Some(Commands::NewPost(options)) => options.run()?,
        Some(Commands::Serve(options)) => options.run()?,
        Some(Commands::About(cmd)) => cmd.run()?,
        None => {
            // Print out the help message since no command was given.
            //
            // FIXME: surely there's a better way to do this...
            Cli::parse_from(["ebg", "--help"].iter());
        }
    }

    Ok(())
}

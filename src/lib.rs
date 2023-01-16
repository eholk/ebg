// FIXME: We don't want to expose the command line directly
// since it depends on clap
pub mod command_line;
pub mod generate;
mod markdown;
mod page;
pub mod site;
mod templates;

use generator::GeneratorError;

// FIXME: We don't want to expose the command line directly
// since it depends on clap
pub mod command_line;
pub mod generator;
mod markdown;
mod page;
pub mod site;
mod templates;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct Error {
    source: InnerError,
}

#[derive(thiserror::Error, Debug)]
enum InnerError {
    #[error("generating site")]
    Generator(
        #[source]
        #[from]
        GeneratorError,
    ),
}

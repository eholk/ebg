use generator::GeneratorError;

pub mod generator;
mod markdown;
pub mod page;
pub mod site;
mod theme;

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

impl From<InnerError> for Error {
    fn from(source: InnerError) -> Self {
        Self { source }
    }
}

impl From<GeneratorError> for Error {
    fn from(value: GeneratorError) -> Self {
        InnerError::Generator(value).into()
    }
}

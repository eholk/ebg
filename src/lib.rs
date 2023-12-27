//! This crate provides the core functionality of Eric's Blog Generator.
//!
//! It is primarily meant to be driven by the `ebg` binary, but in theory it can
//! be used as a library if you try hard enough.
//!
//! The site generator process goes through several phases:
//!
//! 1. Indexing
//! 2. Rendering
//! 3. Generation
//!
//! In a more traditional compiler, these phases correspond roughly to parsing,
//! compilation, and linking.
//!
//! The program is largely serial right now, but the hope is it can be pipelined
//! and parallelized to be an exceptionally fast site generator.
//!
//! ## Indexing
//!
//! The indexing phase is responsible for reading the site's configuration and
//! all the source files. The end result is a data structure that can be used to
//! compute metadata about the site, links between pages, etc.
//!
//! ## Rendering
//!
//! The rendering phase is responsible for taking any markdown source pages and generating the HTML
//! for them.
//!
//! ## Generation
//!
//! The final step is to write out all the rendered contents into the
//! destination directory. Many files are copied directly, but this also
//! generates HTML pages from the rendered markdown contents of the last phase.

use generator::GeneratorError;
use miette::Diagnostic;

pub mod index;
pub mod renderer;
pub mod generator;

mod diagnostics;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug, Diagnostic)]
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

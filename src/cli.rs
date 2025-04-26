//! Code for implementing the command line interface to EBG.

pub mod about;
pub mod build;
pub mod list;
pub mod new_post;

/// Describes a command that can be run from the command line.
///
/// This is normally implemented on the arguments struct.
pub trait Command {
    fn run(self) -> miette::Result<()>;
}

use std::path::PathBuf;

use clap::Args;
use clap::ValueHint::DirPath;

#[derive(Args)]
pub struct Options {
    #[arg(value_hint = DirPath)]
    pub path: PathBuf,

    #[arg(long, short = 'o', value_hint = DirPath, default_value = "publish")]
    pub destination: PathBuf,

    /// Include posts marked with `published: false`
    #[arg(long, default_value_t = false)]
    pub unpublished: bool,
}

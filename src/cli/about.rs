use clap::Parser;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use super::Command;

#[derive(Parser)]
pub struct AboutOptions;

impl Command for AboutOptions {
    fn run(self) -> miette::Result<()> {
        println!("# Syntax Highlighting #");
        println!();
        println!("## Languages ##");
        println!();
        let ss = SyntaxSet::load_defaults_newlines();
        for (i, lang) in ss.syntaxes().iter().enumerate() {
            println!(
                "{}: {} ({})",
                i + 1,
                lang.name,
                lang.file_extensions.join(", ")
            );
        }
        println!();
        println!();
        println!("## Themes ##");
        println!();
        let ts = ThemeSet::load_defaults();
        for (i, theme) in ts.themes.keys().enumerate() {
            println!("{}: {theme}", i + 1);
        }
        println!();
        Ok(())
    }
}

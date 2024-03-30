use std::path::PathBuf;

use clap::{Args, ValueEnum};
use ebg::index::{PageKind, SiteIndex};
use miette::IntoDiagnostic;
use tokio::runtime::Runtime;

use super::{build::find_site_root, Command};

#[derive(Args)]
pub struct ListOptions {
    scope: Scope,
    path: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, PartialEq)]
pub enum Scope {
    All,
    Posts,
    Pages,
    Drafts,
}

impl Command for ListOptions {
    fn run(self) -> miette::Result<()> {
        Runtime::new().into_diagnostic()?.block_on(async move {
            let path = find_site_root(self.path.as_deref())?;

            let site = SiteIndex::from_directory(
                &path,
                self.scope == Scope::Drafts || self.scope == Scope::All,
            )
            .await?;

            let items: Vec<_> = match self.scope {
                Scope::All => site.all_pages().collect(),
                Scope::Posts => site.posts().collect(),
                Scope::Pages => site
                    .all_pages()
                    .filter(|page| page.kind() == PageKind::Page)
                    .collect(),
                Scope::Drafts => site.all_pages().filter(|page| !page.published()).collect(),
            };

            for item in items {
                println!("{}", item.source_path().display());
            }

            Ok(())
        })
    }
}

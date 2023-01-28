use std::{
    io,
    path::{Path, PathBuf},
};

use eyre::Context;
use pathdiff::diff_paths;
use serde_json::{json, Map, Value};
use thiserror::Error;
use tokio::fs;
use tracing::debug;

use crate::{page::Page, site::Site};
use clap::Args;
use clap::ValueHint::DirPath;

use self::atom::generate_atom;

mod atom;

#[derive(Args, Clone)]
pub struct Options {
    #[arg(value_hint = DirPath)]
    pub path: PathBuf,

    #[arg(long, short = 'o', value_hint = DirPath, default_value = "publish")]
    pub destination: PathBuf,

    /// Include posts marked with `published: false`
    #[arg(long, default_value_t = false)]
    pub unpublished: bool,
}

#[derive(Debug, Error)]
pub(crate) enum GeneratorError {
    #[error("generating atom feed")]
    AtomError(#[source] atom::AtomError),
    #[error("could not compute relative path for {0}")]
    ComputeRelativePath(PathBuf),
    #[error("creating destination directory: {}", .0.display())]
    CreateDestDir(PathBuf, #[source] io::Error),
    #[error("copying {} to {}", .0.display(), .1.display())]
    Copy(PathBuf, PathBuf, #[source] io::Error),
    #[error("generating page or post \"{0}\"")]
    // FIXME: use a custom error type here instead of eyre::Report
    GeneratePage(String, #[source] eyre::Report),
    #[error("creating file `{}`", .0.display())]
    CreateFile(PathBuf, #[source] io::Error),
}

pub async fn generate_site(site: &Site, options: &Options) -> super::Result<()> {
    for post in site.all_pages() {
        generate_page(post, site, options)
            .await
            .map_err(|e| GeneratorError::GeneratePage(post.title().to_string(), e))?;
    }

    for file in site.raw_files() {
        debug!(
            "copying from {}, root {}",
            file.display(),
            site.root_dir().display()
        );
        let Some(relative_dest) = diff_paths(file, site.root_dir()) else {
            return Err(GeneratorError::ComputeRelativePath(file.into()))?;
        };
        let dest = options.destination.join(relative_dest);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| GeneratorError::CreateDestDir(parent.into(), e))?;
        }

        fs::copy(file, &dest)
            .await
            .map_err(|e| GeneratorError::Copy(file.into(), dest, e))?;
    }

    generate_atom(
        site,
        std::fs::File::create(options.destination.join("atom.xml"))
            .map_err(|e| GeneratorError::CreateFile("atom.xml".into(), e))?,
    )
    .map_err(GeneratorError::AtomError)?;

    Ok(())
}

/// Converts an object into a format that can be passed to a Tera template
trait ToValue {
    fn value(&self) -> Value;
}

impl ToValue for Page {
    fn value(&self) -> Value {
        let mut page = Map::new();
        page.insert("title".to_string(), json!(self.title()));
        page.insert("url".to_string(), json!(Path::new("/").join(self.url())));
        if let Some(date) = self.publish_date() {
            page.insert("date".to_string(), json!(date));
        }
        page.insert(
            "excerpt".to_string(),
            json!(self.rendered_excerpt().unwrap_or(self.rendered_contents())),
        );
        page.insert("content".to_string(), json!(self.rendered_contents()));
        page.into()
    }
}

impl ToValue for Site {
    fn value(&self) -> Value {
        let mut site = [("url".to_string(), json!(self.base_url()))]
            .into_iter()
            .collect::<Map<_, _>>();

        let mut posts = self.posts().collect::<Vec<_>>();
        posts.sort_by_key(|b| std::cmp::Reverse(b.publish_date()));

        site.insert(
            "posts".to_string(),
            json!(posts.into_iter().map(ToValue::value).collect::<Vec<_>>()),
        );
        site.into()
    }
}

async fn generate_page(page: &Page, site: &Site, options: &Options) -> eyre::Result<()> {
    debug!(
        "post frontmatter:\n{}\n\nparsed as: {:#?}",
        page.raw_frontmatter().unwrap_or("None"),
        page.frontmatter(),
    );

    let dest = options.destination.join(page.url()).join("index.html");

    debug!("destination path: {}", dest.display());

    let content = page.rendered_contents();

    debug!("post template: {:?}", page.template());
    let content = match page.template() {
        Some(template) => {
            let mut context = tera::Context::new();
            context.insert("site", &site.value());
            context.insert("page", &page.value());

            let content_template = site
                .config()
                .macros
                .iter()
                .map(|(name, path)| format!("{{% import \"{}\" as {name} %}}", path.display()))
                .collect::<Vec<_>>()
                .join("")
                + content;
            let mut templates = site.templates().clone();
            let content = templates.render_str(&content_template, &context)?;

            context.insert("content", &content);
            site.templates()
                .render(&format!("{template}.html"), &context)
                .context("rendering template")?
        }
        None => content.to_string(),
    };

    tokio::fs::create_dir_all(dest.parent().unwrap())
        .await
        .context("creating destination directory")?;

    tokio::fs::write(dest, content)
        .await
        .context("writing output")?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{
        generator::ToValue,
        markdown::CodeFormatter,
        page::{Page, SourceFormat},
    };

    /// Regression test for #12
    #[test]
    fn template_full_excerpt_when_missing_delimiter() {
        let mut page = Page::from_string(
            "2012-10-14-hello.md",
            SourceFormat::Markdown,
            "---
title: Hello
layout: page
---
this is *an excerpt*

this is *also an excerpt*",
        );

        page.render(&CodeFormatter::new());

        let value = page.value();

        assert_eq!(
            value["excerpt"],
            "<p>this is <em>an excerpt</em></p>\n<p>this is <em>also an excerpt</em></p>\n<hr />\n"
        );
    }
}

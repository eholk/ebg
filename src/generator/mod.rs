use std::{
    io,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use pathdiff::diff_paths;
use serde_json::{json, Map, Value};
use tera::Tera;
use thiserror::Error;
use tokio::fs;
use tracing::debug;

use crate::{
    index::{PageMetadata, SiteMetadata},
    renderer::{RenderedPageRef, RenderedSite},
};
use clap::Args;
use clap::ValueHint::DirPath;

use self::{atom::generate_atom, theme::create_template_engine};

mod atom;
mod theme;

#[derive(Args, Clone)]
pub struct Options {
    #[arg(value_hint = DirPath)]
    pub path: Option<PathBuf>,

    #[arg(long, short = 'o', value_hint = DirPath, default_value = "publish")]
    pub destination: PathBuf,

    /// Include posts marked with `published: false`
    #[arg(long, default_value_t = false)]
    pub unpublished: bool,
}

#[derive(Diagnostic, Debug, Error)]
pub enum GeneratorError {
    #[error("generating atom feed")]
    AtomError(#[source] atom::AtomError),
    #[error("could not compute relative path for {0}")]
    ComputeRelativePath(PathBuf),
    #[error("removing old destination directory: {}", .0.display())]
    CleanDestDir(PathBuf, #[source] io::Error),
    #[error("creating destination directory: {}", .0.display())]
    CreateDestDir(PathBuf, #[source] io::Error),
    #[error("copying {} to {}", .0.display(), .1.display())]
    Copy(PathBuf, PathBuf, #[source] io::Error),
    #[error("creating file `{}`", .0.display())]
    CreateFile(PathBuf, #[source] io::Error),
    #[error("writing file contents to `{}`", .0.display())]
    WriteFile(PathBuf, #[source] io::Error),
    #[error("loading templates")]
    LoadTemplates(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("importing site macros")]
    ImportSiteMacros(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("rendering template")]
    RenderTemplate(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait Observer: Send + Sync {
    fn begin_load_site(&self) {}
    fn end_load_site(&self, _site: &dyn SiteMetadata) {}
    fn begin_page(&self, _page: &dyn PageMetadata) {}
    fn end_page(&self, _page: &dyn PageMetadata) {}
    fn site_complete(&self, _site: &dyn SiteMetadata) {}
}

/// Holds dynamic state and configuration needed to render a site.
pub struct GeneratorContext<'a> {
    templates: Tera,
    options: &'a Options,
    progress: Option<&'a dyn Observer>,
}

impl<'a> GeneratorContext<'a> {
    pub fn new(site: &RenderedSite, options: &'a Options) -> Result<Self, GeneratorError> {
        let templates = create_template_engine(site.root_dir(), site.config())?;
        Ok(Self {
            templates,
            options,
            progress: None,
        })
    }

    pub fn with_progress(mut self, progress: &'a dyn Observer) -> Self {
        self.progress = Some(progress);
        self
    }

    pub async fn generate_site(&self, site: &RenderedSite<'_>) -> super::Result<()> {
        // Clear the destination directory
        let cleanup = if self.options.destination.exists() {
            let old = tempfile::tempdir().unwrap();
            debug!(
                "moving old destination directory out of the way: {} → {}",
                self.options.destination.display(),
                old.path().display()
            );
            fs::rename(&self.options.destination, &old.path().join("publish"))
                .await
                .map_err(|e| GeneratorError::CleanDestDir(self.options.destination.clone(), e))?;
            Some(tokio::spawn(async move {
                drop(old);
            }))
        } else {
            None
        };

        // Create the destination directory
        tokio::fs::create_dir_all(&self.options.destination)
            .await
            .map_err(|e| GeneratorError::CreateDestDir(self.options.destination.clone(), e))?;

        // Generate pages
        for post in site.all_pages() {
            if let Some(progress) = self.progress {
                progress.begin_page(&post);
            }
            self.generate_page(post, site)?;
            if let Some(progress) = self.progress {
                progress.end_page(&post);
            }
        }

        // Copy raw files (those that don't need processing or generation)
        for file in site.raw_files() {
            debug!(
                "copying from {}, root {}",
                file.display(),
                site.root_dir().display()
            );
            let Some(relative_dest) = diff_paths(file, site.root_dir()) else {
                return Err(GeneratorError::ComputeRelativePath(file.into()))?;
            };
            let dest = self.options.destination.join(relative_dest);

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| GeneratorError::CreateDestDir(parent.into(), e))?;
            }

            fs::copy(file, &dest)
                .await
                .map_err(|e| GeneratorError::Copy(file.into(), dest, e))?;
        }

        // Generate the atom feed
        //
        // FIXME: this is only relevant if we have posts. Maybe it should have an option to disable it
        // in the site config?
        generate_atom(
            site,
            std::fs::File::create(self.options.destination.join("atom.xml"))
                .map_err(|e| GeneratorError::CreateFile("atom.xml".into(), e))?,
        )
        .map_err(GeneratorError::AtomError)?;

        if let Some(cleanup) = cleanup {
            cleanup.await.unwrap()
        }

        Ok(())
    }

    fn generate_page(
        &self,
        page: RenderedPageRef<'_>,
        site: &RenderedSite<'_>,
    ) -> Result<(), GeneratorError> {
        let dest = self.options.destination.join(page.url()).join("index.html");

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
                let mut templates = self.templates.clone();
                let content = templates
                    .render_str(&content_template, &context)
                    .map_err(|e| GeneratorError::ImportSiteMacros(Box::new(e)))?;

                context.insert("content", &content);
                self.templates
                    .render(&format!("{template}.html"), &context)
                    .map_err(|e| GeneratorError::RenderTemplate(Box::new(e)))?
            }
            None => content.to_string(),
        };

        std::fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| GeneratorError::CreateDestDir(dest.parent().unwrap().to_path_buf(), e))?;

        std::fs::write(&dest, content).map_err(|e| GeneratorError::WriteFile(dest, e))?;

        Ok(())
    }
}

/// Converts an object into a format that can be passed to a Tera template
trait ToValue {
    fn value(&self) -> Value;
}

impl ToValue for RenderedPageRef<'_> {
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

impl ToValue for RenderedSite<'_> {
    fn value(&self) -> Value {
        let mut site = [("url".to_string(), json!(self.base_url()))]
            .into_iter()
            .collect::<Map<_, _>>();

        let mut posts = self.posts().collect::<Vec<_>>();
        posts.sort_by_key(|b| std::cmp::Reverse(b.publish_date()));

        site.insert(
            "posts".to_string(),
            json!(posts
                .into_iter()
                .map(|post| post.value())
                .collect::<Vec<_>>()),
        );
        site.into()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        index::{PageSource, SiteIndex, SourceFormat},
        renderer::{CodeFormatter, RenderContext, RenderSource, RenderedPageRef},
    };

    use super::ToValue;

    /// Regression test for #12
    #[test]
    fn template_full_excerpt_when_missing_delimiter() -> miette::Result<()> {
        let page = PageSource::from_string(
            "2012-10-14-hello.md",
            SourceFormat::Markdown,
            "---
title: Hello
layout: page
---
this is *an excerpt*

this is *also an excerpt*",
        );

        let site = SiteIndex::default();
        let fmt = CodeFormatter::new();
        let rcx = RenderContext::new(&site, &fmt);
        let rendered_page = page.render(&rcx)?;
        let page = RenderedPageRef::new(&page, &rendered_page);
        let page = page.value();

        assert_eq!(
            page["excerpt"],
            "<p>this is <em>an excerpt</em></p>\n<p>this is <em>also an excerpt</em></p>\n<hr />\n"
        );

        Ok(())
    }
}

use std::sync::Mutex;

use miette::Diagnostic;
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    diagnostics::{DiagnosticContext, ErrorSet},
    index::{Category, PageMetadata, PageSource, SiteIndex, SiteMetadata, SourceFormat},
};

use self::markdown::render_markdown;

mod markdown;

pub(crate) use self::markdown::CodeFormatter;

/// Contains all the generated contents of a site
///
/// Mainly this means all pages with their markdown converted to HTML.
pub struct RenderedSite<'a> {
    source: &'a SiteIndex,
    pages: Vec<RenderedPage>,
}

impl<'a> RenderedSite<'a> {
    pub fn all_pages(&self) -> impl Iterator<Item = RenderedPageRef<'_>> {
        self.pages
            .iter()
            .zip(self.source.all_pages())
            .map(move |(page, source)| RenderedPageRef::new(source, page))
    }

    pub fn posts(&self) -> impl Iterator<Item = RenderedPageRef<'_>> {
        self.source
            .all_pages()
            .zip(self.all_pages())
            .filter(|(page, _)| page.is_post())
            .map(|(_, page)| page)
    }

    pub fn categories_and_pages(
        &self,
    ) -> impl Iterator<Item = (&Category, impl Iterator<Item = RenderedPageRef<'_>>)> {
        self.source.categories().map(|category| {
            let pages = self.all_pages().filter(|page| {
                page.source
                    .categories()
                    .map_or(false, |mut c| c.any(|cat| cat == category.name))
            });
            (category, pages)
        })
    }
}

impl<'a> SiteMetadata for RenderedSite<'a> {
    fn config(&self) -> &crate::index::Config {
        self.source.config()
    }

    fn base_url(&self) -> &str {
        self.source.base_url()
    }

    fn title(&self) -> &str {
        self.source.title()
    }

    fn subtitle(&self) -> Option<&str> {
        self.source.subtitle()
    }

    fn author(&self) -> Option<&str> {
        self.source.author()
    }

    fn author_email(&self) -> Option<&str> {
        self.source.author_email()
    }

    fn root_dir(&self) -> &std::path::PathBuf {
        self.source.root_dir()
    }

    fn num_pages(&self) -> usize {
        self.source.num_pages()
    }

    fn raw_files(&self) -> impl Iterator<Item = &std::path::Path> {
        self.source.raw_files()
    }
}

impl SiteIndex {
    pub fn render(&self) -> Result<RenderedSite, RenderError> {
        let code_formatter = CodeFormatter::new();
        let pages = RenderContext::run_dcx(&self, &code_formatter, |ctx| {
            self.all_pages()
                .collect::<Vec<_>>()
                .par_iter()
                .map(|page| page.render(&ctx))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(RenderError::PageRenderingErrors)?;
        Ok(RenderedSite {
            source: self,
            pages,
        })
    }
}

#[derive(Clone, Copy)]
pub struct RenderedPageRef<'a> {
    pub(crate) source: &'a PageSource,
    pub(crate) page: &'a RenderedPage,
}

impl<'a> RenderedPageRef<'a> {
    pub(crate) fn new(source: &'a PageSource, page: &'a RenderedPage) -> Self {
        Self { source, page }
    }

    pub fn title(&self) -> &str {
        self.page.title()
    }

    pub fn rendered_contents(&self) -> &str {
        self.page.rendered_contents()
    }

    pub fn rendered_excerpt(&self) -> Option<&str> {
        self.page.rendered_excerpt()
    }
}

impl<'a> PageMetadata for RenderedPageRef<'a> {
    fn url(&self) -> String {
        self.source.url()
    }

    fn publish_date(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.source.publish_date()
    }

    fn template(&self) -> Option<&str> {
        self.source.template()
    }
}

/// Represents parts of the page that are computed during site generation.
///
/// Mainly this includes the rendered contents of the page.
pub struct RenderedPage {
    /// The contents of this page rendered as HTML
    rendered_contents: String,
    /// The title that comes from the content if it is markdown and starts with an h1.
    ///
    /// Filled in by [Page::render].
    content_title: String,
}

impl RenderedPage {
    pub fn title(&self) -> &str {
        &self.content_title
    }

    pub fn rendered_contents(&self) -> &str {
        self.rendered_contents.as_str()
    }

    pub fn rendered_excerpt(&self) -> Option<&str> {
        let (excerpt, rest) = self.rendered_contents().split_once("<!--")?;
        let (comment, _) = rest.split_once("-->")?;
        (comment.trim() == "MORE").then_some(excerpt)
    }
}

/// Holds dynamic state and configuration needed to render a site.
pub struct RenderContext<'a> {
    site: &'a SiteIndex,
    code_formatter: &'a CodeFormatter,
    dcx: Mutex<&'a mut DiagnosticContext>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        site: &'a SiteIndex,
        code_formatter: &'a CodeFormatter,
        dcx: &'a mut DiagnosticContext,
    ) -> Self {
        Self {
            site,
            code_formatter,
            dcx: dcx.into(),
        }
    }

    pub fn run_dcx<T, E>(
        site: &SiteIndex,
        code_formatter: &CodeFormatter,
        f: impl FnOnce(&RenderContext<'_>) -> Result<T, E>,
    ) -> Result<T, ErrorSet>
    where
        E: Diagnostic + Send + Sync + 'static,
    {
        DiagnosticContext::with(|dcx| {
            let dcx = dcx.into();
            let mut rcx = RenderContext {
                site,
                code_formatter,
                dcx,
            };
            f(&mut rcx)
        })
    }

    /// Runs a closure under a new diagnostic context
    pub fn run_with_new_dcx<T, E>(
        &self,
        f: impl FnOnce(&RenderContext<'_>) -> Result<T, E>,
    ) -> Result<T, ErrorSet>
    where
        E: Diagnostic + Send + Sync + 'static,
    {
        Self::run_dcx(self.site, self.code_formatter, f)
    }
}

pub trait RenderSource {
    /// Renders the source to HTML
    fn render(&self, ctx: &RenderContext) -> Result<RenderedPage, RenderError>;
}

impl RenderSource for PageSource {
    fn render(&self, rcx: &RenderContext) -> Result<RenderedPage, RenderError> {
        Ok(match self.source_format() {
            SourceFormat::Html => RenderedPage {
                rendered_contents: self.mainmatter().to_string(),
                // FIXME: generate a title from the filename or something if there's no title given
                content_title: self.title().unwrap_or("⛔Untitled⛔").to_string(),
            },
            SourceFormat::Markdown => {
                let (rendered_contents, content_title) = render_markdown(self, rcx);
                let content_title = content_title
                    .or_else(|| self.title().map(ToString::to_string))
                    // FIXME: generate a title from the filename or something if there's no title given
                    .unwrap_or("⛔Untitled⛔".to_string());
                RenderedPage {
                    rendered_contents,
                    content_title,
                }
            }
        })
    }
}

/// Describes a failure to render something
#[derive(Diagnostic, Debug, Error)]
pub enum RenderError {
    #[error("failed to render pages")]
    PageRenderingErrors(ErrorSet),
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::{
        index::{PageSource, SiteIndex, SourceFormat},
        renderer::{RenderContext, RenderSource, markdown::CodeFormatter},
    };

    #[test]
    fn rendered_excerpt() -> miette::Result<()> {
        let page = PageSource::from_string(
            "2012-10-14-hello.md",
            SourceFormat::Markdown,
            "---
title: Hello
layout: page
---
this is *an excerpt*
<!-- MORE -->
this is *not an excerpt*",
        );

        let site = SiteIndex::default();
        let code_formatter = CodeFormatter::new();
        let page = RenderContext::run_dcx(&site, &code_formatter, |rcx| page.render(&rcx))?;

        assert_eq!(
            page.rendered_excerpt(),
            Some("<p>this is <em>an excerpt</em></p>\n")
        );

        Ok(())
    }

    #[test]
    fn leading_h1_as_title() -> miette::Result<()> {
        const SRC: &str = r#"---
layout: post
title: "Hello, World!"
date: 2012-01-07 14:40
comments: true
categories:
---

# This is the title
"#;
        let post = PageSource::from_string(
            "_posts/2012-01-07-hello-world.md",
            SourceFormat::Markdown,
            SRC,
        );
        let site = SiteIndex::default();
        let code_formatter = CodeFormatter::new();
        let post = RenderContext::run_dcx(&site, &code_formatter, |rcx| post.render(&rcx))?;
        assert_eq!(post.title(), "This is the title");
        Ok(())
    }

    #[test]
    fn resolve_source_link() -> miette::Result<()> {
        let mut site = SiteIndex::default();
        site.add_page(PageSource::from_string(
            "_posts/2012-10-14-hello.md",
            SourceFormat::Markdown,
            "",
        ));
        site.add_page(PageSource::from_string(
            "_posts/2013-10-14-page2.md",
            SourceFormat::Markdown,
            "[hello](./2012-10-14-hello.md)",
        ));
        let code_formatter = CodeFormatter::new();

        let render_page = site
            .find_page_by_source_path(&PathBuf::from("_posts/2013-10-14-page2.md"))
            .unwrap();

        let rendered_page =
            RenderContext::run_dcx(&site, &code_formatter, |rcx| render_page.render(&rcx))?;

        assert_eq!(
            rendered_page.rendered_contents(),
            "<p><a href=\"/blog/2012/10/14/hello/\">hello</a></p>\n<hr />\n"
        );

        Ok(())
    }

    /// Make sure that source links handle anchors correctly
    #[test]
    fn resolve_source_link_hashtag() -> miette::Result<()> {
        let mut site = SiteIndex::default();
        site.add_page(PageSource::from_string(
            "_posts/2012-10-14-hello.md",
            SourceFormat::Markdown,
            "",
        ));
        site.add_page(PageSource::from_string(
            "_posts/2013-10-14-page2.md",
            SourceFormat::Markdown,
            "[hello](./2012-10-14-hello.md#title)",
        ));
        let code_formatter = CodeFormatter::new();
        let render_page = site
            .find_page_by_source_path(&PathBuf::from("_posts/2013-10-14-page2.md"))
            .unwrap();

        let rendered_page =
            RenderContext::run_dcx(&site, &code_formatter, |rcx| render_page.render(&rcx))?;
        assert_eq!(
            rendered_page.rendered_contents(),
            "<p><a href=\"/blog/2012/10/14/hello/#title\">hello</a></p>\n<hr />\n"
        );

        Ok(())
    }
}

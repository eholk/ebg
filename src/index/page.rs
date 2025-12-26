//! Data structures representing a page.

use std::{
    ffi::OsStr,
    ops::{Range, RangeFrom},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, Local, TimeZone, Utc};
use miette::Diagnostic;
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Deserialize;
use thiserror::Error;
use tokio::fs::read_to_string;
use tracing::debug;
use url::Url;

use self::parsing_helpers::{
    deserialize_comma_separated_list, deserialize_date_opt, find_frontmatter_delimiter,
};
use super::LinkDest;

pub(crate) mod parsing_helpers;

type Date = DateTime<Utc>;

#[derive(Deserialize, Debug)]
pub struct FrontMatter {
    layout: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_date_opt")]
    date: Option<Date>,
    #[allow(unused)]
    comments: Option<bool>,
    #[serde(default)]
    categories: Vec<String>,
    #[allow(unused)]
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_comma_separated_list")]
    tags: Vec<String>,
    #[serde(rename = "external-url")]
    #[allow(dead_code)] // FIXME: remove this when we start using this
    external_url: Option<String>,
    #[allow(dead_code)] // FIXME: remove this when we start using this
    permalink: Option<String>,
    #[serde(default = "default_true")]
    published: bool,
    #[serde(default = "default_true")]
    show_in_home: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SourceFormat {
    Html,
    Markdown,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum PageKind {
    Page,
    Post,
}

#[derive(Diagnostic, Debug, Error)]
pub enum PageLoadError {
    #[error("could not interpret filename")]
    ParseFilename(#[diagnostic_source] ParseFilenameError),

    #[error("reading post contents")]
    #[diagnostic(severity(error))]
    ReadingPostContents(#[source] std::io::Error),
}

/// Represents the content of a page that can be trivially read from disk
///
/// This includes metadata that is often helpful in rendering other pages but
/// notably does not include anything that requires the page itself to be
/// rendered.
pub struct PageSource {
    kind: PageKind,
    format: SourceFormat,
    source: PathBuf,
    contents: String,
    frontmatter: Option<Range<usize>>,
    mainmatter: RangeFrom<usize>,
    parsed_frontmatter: Option<FrontMatter>,
}

impl PageSource {
    /// Reads the file `filename` into a `Page`
    ///
    /// The `root_dir` specifies the root directory for the site. This page will
    /// be given a path relative to the root directory.
    pub async fn from_file(
        filename: impl Into<PathBuf>,
        root_dir: &Path,
    ) -> Result<Self, PageLoadError> {
        let filename: PathBuf = filename.into();

        let (_, kind, _) = parse_filename(&filename).map_err(PageLoadError::ParseFilename)?;

        let contents = read_to_string(&filename)
            .await
            .map_err(PageLoadError::ReadingPostContents)?;
        Ok(Self::from_string(
            pathdiff::diff_paths(filename, root_dir).unwrap(),
            kind,
            contents,
        ))
    }

    pub fn from_string(
        source: impl Into<PathBuf>,
        format: SourceFormat,
        contents: impl ToString,
    ) -> Self {
        let source = source.into();
        debug!("creating page with source path `{}`", source.display());
        let contents = contents.to_string();
        // FIXME: we need to determine the kind more precisely, since we might be loading from a
        // directory other than _posts
        let kind = if source.components().next().unwrap().as_os_str() == OsStr::new("_posts") {
            PageKind::Post
        } else {
            PageKind::Page
        };
        let frontmatter = find_frontmatter_delimiter(&contents).and_then(|range| {
            let start = range.end;
            let ending_delimiter = find_frontmatter_delimiter(&contents[start..])?;
            Some((
                start..(start + ending_delimiter.start),
                (start + ending_delimiter.end)..,
            ))
        });
        let (frontmatter, mainmatter) = match frontmatter {
            Some((frontmatter, mainmatter)) => (Some(frontmatter), mainmatter),
            None => (None, 0..),
        };

        let parsed_frontmatter = frontmatter
            .as_ref()
            .and_then(|frontmatter| serde_yaml::from_str(&contents[frontmatter.clone()]).ok());

        Self {
            kind,
            format,
            source,
            contents,
            frontmatter,
            mainmatter,
            parsed_frontmatter,
        }
    }

    pub fn raw_frontmatter(&self) -> Option<&str> {
        self.frontmatter
            .as_ref()
            .map(|frontmatter| &self.contents[frontmatter.clone()])
    }

    pub fn frontmatter(&self) -> Option<&FrontMatter> {
        self.parsed_frontmatter.as_ref()
    }

    pub fn mainmatter(&self) -> &str {
        &self.contents[self.mainmatter.clone()]
    }

    /// Returns the title from the frontmatter, if one is given.
    pub fn title(&self) -> Option<&str> {
        self.frontmatter()
            .map(|frontmatter| frontmatter.title.as_str())
    }

    pub fn title_slug(&self) -> &str {
        let (_, _, slug) = parse_filename(&self.source).unwrap();
        slug
    }

    pub fn source_format(&self) -> SourceFormat {
        self.format
    }

    pub fn kind(&self) -> PageKind {
        self.kind
    }

    pub fn is_post(&self) -> bool {
        self.kind == PageKind::Post
    }

    pub fn published(&self) -> bool {
        self.parsed_frontmatter
            .as_ref()
            .map(|front| front.published)
            .unwrap_or(true)
    }

    /// Returns the path to this page's source file relative to the site root.
    pub fn source_path(&self) -> &Path {
        self.source.as_path()
    }

    pub fn categories(&self) -> Option<impl Iterator<Item = &str>> {
        self.frontmatter()
            .map(|frontmatter| frontmatter.categories.iter().map(|s| s.as_str()))
    }

    pub fn show_in_home(&self) -> bool {
        self.frontmatter()
            .map(|front| front.show_in_home)
            .unwrap_or(true)
    }

    /// Extracts external links from the page's markdown content.
    ///
    /// This parses the markdown and returns all links that appear to be
    /// external URLs (not relative paths, fragments, or email addresses).
    pub fn external_links(&self) -> impl Iterator<Item = Url> {
        let mut links = Vec::new();

        // Only process markdown content
        if self.format != SourceFormat::Markdown {
            return links.into_iter();
        }

        let markdown = self.mainmatter();
        let parser = Parser::new_ext(markdown, Options::all());

        for event in parser {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                // Parse the link to classify it - only include external http/https URLs
                if let Ok(LinkDest::External(url)) = LinkDest::parse(dest_url.as_ref()) {
                    if url.scheme() == "http" || url.scheme() == "https" {
                        links.push(url);
                    }
                }
            }
        }

        links.into_iter()
    }
}

pub trait PageMetadata {
    fn url(&self) -> String; // TODO: return a URL type instead.

    /// Returns the date and time the post was published.
    ///
    /// If the data is specified in the frontmatter, that date will be used,
    /// otherwise the date will be inferred from the file name.
    fn publish_date(&self) -> Option<Date>;

    /// Returns the name of the template that should be used with this page.
    fn template(&self) -> Option<&str>;
}

impl PageMetadata for PageSource {
    fn url(&self) -> String {
        match self.kind {
            PageKind::Post => match self.publish_date() {
                Some(date) => Path::new("blog")
                    .join(date.year().to_string())
                    .join(format!("{:02}", date.month()))
                    .join(format!("{:02}", date.day()))
                    .join(self.title_slug().to_string() + "/"),
                None => Path::new("blog").join(self.title_slug()),
            },
            PageKind::Page => url_from_page_path(&self.source),
        }
        .to_string_lossy()
        .replace('\\', "/")
    }

    fn publish_date(&self) -> Option<Date> {
        let from_filename = {
            let (date, _, _) = parse_filename(&self.source).unwrap();
            Some(date)
        };
        self.parsed_frontmatter
            .as_ref()
            .and_then(|frontmatter| frontmatter.date)
            .or(from_filename)
    }

    fn template(&self) -> Option<&str> {
        self.parsed_frontmatter
            .as_ref()
            .map(|frontmatter| frontmatter.layout.as_str())
    }
}

fn url_from_page_path(path: &Path) -> PathBuf {
    if path.file_stem().unwrap() == "index" {
        path.parent().unwrap_or(Path::new("")).to_path_buf()
    } else {
        path.parent()
            .unwrap_or(Path::new(""))
            .join(path.file_stem().unwrap_or(OsStr::new("")))
    }
}

#[derive(Debug, Diagnostic, Error, PartialEq)]
pub enum ParseFilenameError {
    #[error("filename has no extension")]
    #[diagnostic(help("make sure the file extension is .md or .html"))]
    NoExtension,
    #[error("unrecognized extension")]
    #[diagnostic(help("try adding a .md or .html extension"))]
    UnrecognizedExtension {
        #[source_code]
        filename: String,
        #[label("this extension is not recognized")]
        span: Range<usize>,
    },
}

/// Extracts the publish date, page kind, and title from a path like
/// `_posts/2022-10-14-hello-world.md`, or returns None if the file doesn't match
/// the expected format.
/// Also handles directory-based posts like `_posts/2022-10-14-hello-world/index.md`
fn parse_filename(path: &Path) -> Result<(Date, SourceFormat, &str), ParseFilenameError> {
    let kind = match path.extension().and_then(|ext| ext.to_str()) {
        Some("md" | "markdown") => SourceFormat::Markdown,
        Some("html" | "htm") => SourceFormat::Html,
        Some(_) => {
            let path = path.to_str().unwrap();
            let ext_loc = path.rfind('.').unwrap() + 1;
            let span = ext_loc..(path.len());
            return Err(ParseFilenameError::UnrecognizedExtension {
                filename: path.into(),
                span,
            });
        }
        None => return Err(ParseFilenameError::NoExtension),
    };

    // FIXME: replace unwraps with diagnostics to explain why the date is wrong.
    let filename = path.file_stem().unwrap().to_str().unwrap();

    // Check if this is an index.md file in a directory
    // If so, try to parse the date from the parent directory name instead of "index"
    // This enables directory-based posts like: _posts/2023-11-08-post-name/index.md
    let name_to_parse = if filename == "index" {
        // For index.md files, use the parent directory name for date/slug extraction
        // If we can't get the parent directory name, fall back to "index" (which won't have a date)
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(filename)
    } else {
        filename
    };

    match parse_date_from_filename(name_to_parse) {
        Some((date, rest)) => Ok((date, kind, rest)),
        None => Ok((
            // FIXME: We should return an option rather than fabricating a date
            DateTime::from_timestamp_millis(0).unwrap(),
            kind,
            name_to_parse,
        )),
    }
}

/// Attempts to parse a date from a file name and returns the date with the
/// remainder of the filename
fn parse_date_from_filename(filename: &str) -> Option<(Date, &str)> {
    let (year, rest) = filename.split_once('-')?;
    let (month, rest) = rest.split_once('-')?;
    let (day, rest) = rest.split_once('-')?;
    Some((
        Local
            .with_ymd_and_hms(
                year.parse().ok()?,
                month.parse().ok()?,
                day.parse().ok()?,
                0,
                0,
                0,
            )
            .single()?
            .with_timezone(&Utc),
        rest,
    ))
}

#[cfg(test)]
mod test {
    use crate::index::{SourceFormat, page::PageMetadata};

    use super::{FrontMatter, PageSource, parse_filename};
    use chrono::{DateTime, Local, TimeZone, Utc};
    use miette::IntoDiagnostic;
    use std::path::Path;

    #[test]
    fn parse_bare_filename() {
        assert_eq!(
            parse_filename(Path::new("about.md")),
            Ok((
                DateTime::from_timestamp_millis(0).unwrap(),
                SourceFormat::Markdown,
                "about"
            ))
        );
    }

    #[test]
    fn parse_post_filename() {
        assert_eq!(
            parse_filename(
                &Path::new("_posts").join("2021-01-14-coming-soon-primitive-computing.md")
            ),
            Ok((
                Local
                    .with_ymd_and_hms(2021, 1, 14, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Markdown,
                "coming-soon-primitive-computing"
            ))
        );
    }

    #[test]
    fn primitive_computing_post() {
        let post = PageSource::from_string(
            Path::new("_posts").join("2021-01-14-coming-soon-primitive-computing.md"),
            SourceFormat::Markdown,
            "---\nlayout: post
title: \"Coming Soon: Primitive Computing\"
comments: true
---

Coming soon!
",
        );

        assert_eq!(
            post.publish_date(),
            Some(
                Local
                    .with_ymd_and_hms(2021, 1, 14, 0, 0, 0)
                    .single()
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
    }

    #[test]
    fn url_from_path_path() {
        assert_eq!(
            super::url_from_page_path(Path::new("about.md")),
            Path::new("about/")
        );

        assert_eq!(
            super::url_from_page_path(Path::new("archive/index.html")),
            Path::new("archive/")
        );
    }

    #[test]
    fn parse_frontmatter() -> miette::Result<()> {
        const SRC: &str = r#"layout: post
title: "Hello, World!"
date: 2012-11-27 19:40
comments: true
categories:
"#;

        let front: FrontMatter = serde_yaml::from_str(SRC).into_diagnostic()?;

        // TODO: make sure we actually parsed the right values
        println!("{front:?}");

        Ok(())
    }

    #[test]
    fn parse_contents_with_frontmatter() {
        const SRC: &str = r#"---
layout: post
title: "Hello, World!"
date: 2012-11-27 19:40
comments: true
categories:
---
Hello, world!
"#;
        let post = PageSource::from_string("hello.md", SourceFormat::Markdown, SRC);
        assert_eq!(
            post.raw_frontmatter(),
            Some(
                r#"layout: post
title: "Hello, World!"
date: 2012-11-27 19:40
comments: true
categories:
"#
            )
        );

        assert_eq!(post.mainmatter(), "Hello, world!\n");
    }

    #[test]
    fn url_has_leading_zeroes() {
        const SRC: &str = r#"---
layout: post
title: "Hello, World!"
date: 2012-01-07 14:40
comments: true
categories:
---
Hello, world!
"#;
        let post = PageSource::from_string(
            "_posts/2012-01-07-hello-world.md",
            SourceFormat::Markdown,
            SRC,
        );
        assert_eq!(post.url(), "blog/2012/01/07/hello-world/");
    }

    /// Regression test for #13
    #[test]
    fn url_has_tailing_slash() {
        const SRC: &str = r#"---
layout: post
title: "Hello, World!"
---
Hello, world!
"#;
        let post = PageSource::from_string(
            "_posts/2023-01-24-hello-world.md",
            SourceFormat::Markdown,
            SRC,
        );
        assert_eq!(post.url(), "blog/2023/01/24/hello-world/");
    }

    #[test]
    fn parse_contents_without_frontmatter() {
        const SRC: &str = r#"Hello, world!
"#;
        let post = PageSource::from_string("hello.md", SourceFormat::Markdown, SRC);
        assert_eq!(post.raw_frontmatter(), None);
        assert_eq!(post.mainmatter(), "Hello, world!\n");
    }

    #[test]
    fn parse_contents_with_invalid_frontmatter() {
        const SRC: &str = r#"---
layout: post
title: "Hello, World!"
date: 2012-11-27 19:40
comments: true
categories:

Hello, world!
"#;
        let post = PageSource::from_string("hello.md", SourceFormat::Markdown, SRC);
        assert_eq!(post.raw_frontmatter(), None);
        assert_eq!(post.mainmatter(), SRC);
    }

    #[test]
    fn parse_contents_with_crlf_frontmatter() {
        const SRC: &str = "---\r\nlayout: post\r\ntitle: \"Hello, World!\"\r\ndate: 2012-11-27 19:40\r\ncomments: true\r\ncategories:\r\n---\r\nHello, world!\r\n";
        let post = PageSource::from_string("hello.md", SourceFormat::Markdown, SRC);
        assert_eq!(
            post.raw_frontmatter(),
            Some(
                "layout: post\r\ntitle: \"Hello, World!\"\r\ndate: 2012-11-27 19:40\r\ncomments: true\r\ncategories:\r\n"
            )
        );

        assert_eq!(post.mainmatter(), "Hello, world!\r\n");
    }

    #[test]
    fn parse_filenames() {
        assert!(
            // FIXME: make sure we get the right kind of error
            parse_filename(Path::new("_post/2022-10-14-hello.toml")).is_err()
        );

        assert_eq!(
            parse_filename(Path::new("_post/2022-10-14-hello.md")),
            Ok((
                Local
                    .with_ymd_and_hms(2022, 10, 14, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Markdown,
                "hello"
            ))
        );

        assert_eq!(
            parse_filename(Path::new("_post/2022-10-14-long-file-name.markdown")),
            Ok((
                Local
                    .with_ymd_and_hms(2022, 10, 14, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Markdown,
                "long-file-name"
            ))
        );

        assert_eq!(
            parse_filename(Path::new("_post/2022-10-14-long-file-name.htm")),
            Ok((
                Local
                    .with_ymd_and_hms(2022, 10, 14, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Html,
                "long-file-name"
            ))
        );

        assert_eq!(
            parse_filename(Path::new("_post/2022-10-14-long-file-name.html")),
            Ok((
                Local
                    .with_ymd_and_hms(2022, 10, 14, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Html,
                "long-file-name"
            ))
        );
    }

    #[test]
    fn parse_directory_based_post() {
        // Test parsing a directory-based post like _posts/2022-10-14-hello-world/index.md
        assert_eq!(
            parse_filename(Path::new("_posts/2022-10-14-hello-world/index.md")),
            Ok((
                Local
                    .with_ymd_and_hms(2022, 10, 14, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Markdown,
                "hello-world"
            ))
        );
    }

    #[test]
    fn parse_directory_based_post_with_long_name() {
        assert_eq!(
            parse_filename(Path::new("_posts/2023-11-08-new-post/index.md")),
            Ok((
                Local
                    .with_ymd_and_hms(2023, 11, 8, 0, 0, 0)
                    .unwrap()
                    .with_timezone(&Utc),
                SourceFormat::Markdown,
                "new-post"
            ))
        );
    }

    #[test]
    fn url_for_directory_based_post() {
        const SRC: &str = r#"---
layout: post
title: "Hello, World!"
---
Hello, world!
"#;
        let post = PageSource::from_string(
            "_posts/2023-01-24-hello-world/index.md",
            SourceFormat::Markdown,
            SRC,
        );
        assert_eq!(post.url(), "blog/2023/01/24/hello-world/");
    }

    #[test]
    fn parse_incomplete_frontmatter() {
        let front: Result<FrontMatter, _> = serde_yaml::from_str(
            "layout: page
title: About
permalink: /about/
",
        );
        println!("frontmatter: {front:#?}");
        assert!(front.is_ok());
    }

    #[test]
    fn parse_frontmatter_tags() -> miette::Result<()> {
        let front: FrontMatter = serde_yaml::from_str(
            "layout: page
title: About
tags: tag1, tag2
",
        )
        .into_diagnostic()?;
        println!("frontmatter: {front:#?}");
        assert_eq!(front.tags, vec!["tag1".to_string(), "tag2".to_string()]);
        Ok(())
    }

    #[test]
    fn test_external_links() {
        const SRC: &str = r#"---
layout: post
title: "Test Post"
---

# Test Post

Check out [Rust](https://www.rust-lang.org/) and [Wikipedia](https://en.wikipedia.org/wiki/Rust).

Also see [this local page](/about/) and [another post](../other-post.md).

Email me at [me@example.com](mailto:me@example.com).

Jump to [the section](#section).
"#;
        let post =
            PageSource::from_string("_posts/2023-01-24-test.md", SourceFormat::Markdown, SRC);

        let links = post.external_links().collect::<Vec<_>>();
        assert_eq!(links.len(), 2);
        assert!(
            links
                .iter()
                .any(|u| u.as_str() == "https://www.rust-lang.org/")
        );
        assert!(
            links
                .iter()
                .any(|u| u.as_str() == "https://en.wikipedia.org/wiki/Rust")
        );
    }
}

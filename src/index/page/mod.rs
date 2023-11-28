//! Data structures representing a page.

use std::{
    ffi::OsStr,
    ops::{Range, RangeFrom},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use tokio::fs::read_to_string;

use self::parsing_helpers::{
    deserialize_comma_separated_list, deserialize_date, find_frontmatter_delimiter,
};

use super::IndexError;

mod parsing_helpers;

type Date = DateTime<Utc>;

#[derive(Deserialize, Debug)]
pub struct FrontMatter {
    layout: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_date")]
    date: Option<Date>,
    #[allow(unused)]
    comments: Option<bool>,
    #[allow(unused)]
    categories: Option<Vec<String>>,
    #[allow(unused)]
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_comma_separated_list")]
    tags: Vec<String>,
    #[serde(rename = "external-url")]
    #[allow(dead_code)] // FIXME: remove this when we start using this
    external_url: Option<String>,
    #[allow(dead_code)] // FIXME: remove this when we start using this
    permalink: Option<String>,
    #[serde(default = "mk_true")]
    published: bool,
}

fn mk_true() -> bool {
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
    ) -> Result<Self, IndexError> {
        let filename: PathBuf = filename.into();

        let Some((_, kind, _)) = parse_filename(&filename) else {
            return Err(IndexError::InvalidFilename(filename));
        };

        let contents = read_to_string(&filename)
            .await
            .map_err(IndexError::ReadingPostContents)?;
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

/// Extracts the publish date, page kind, and title from a path like
/// `_posts/2022-10-14-hello-world.md`, or returns None if the file doesn't match
/// the expected format.
fn parse_filename(path: &Path) -> Option<(Date, SourceFormat, &str)> {
    let kind = match path.extension()?.to_str()? {
        "md" | "markdown" => SourceFormat::Markdown,
        "html" | "htm" => SourceFormat::Html,
        _ => return None,
    };

    let filename = path.file_stem()?.to_str()?;
    match parse_date_from_filename(filename) {
        Some((date, rest)) => Some((date, kind, rest)),
        None => Some((
            // FIXME: We should return an option rather than fabricating a date
            NaiveDateTime::from_timestamp_millis(0)?
                .and_local_timezone(Utc)
                .single()?,
            kind,
            filename,
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
    use crate::index::{page::PageMetadata, SourceFormat};

    use super::{parse_filename, FrontMatter, PageSource};
    use chrono::{Local, NaiveDateTime, TimeZone, Utc};
    use miette::IntoDiagnostic;
    use std::path::Path;

    #[test]
    fn parse_bare_filename() {
        assert_eq!(
            parse_filename(Path::new("about.md")),
            Some((
                NaiveDateTime::from_timestamp_millis(0)
                    .unwrap()
                    .and_local_timezone(Utc)
                    .unwrap(),
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
            Some((
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
        assert_eq!(
            parse_filename(Path::new("_post/2022-10-14-hello.toml")),
            None
        );

        assert_eq!(
            parse_filename(Path::new("_post/2022-10-14-hello.md")),
            Some((
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
            Some((
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
            Some((
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
            Some((
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
}

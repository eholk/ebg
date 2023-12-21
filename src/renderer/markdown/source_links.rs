use std::fmt::Formatter;

use miette::Diagnostic;
use pulldown_cmark::{CowStr, Event, Tag};
use thiserror::Error;
use tracing::debug;
use url::Url;

use crate::{
    index::{PageMetadata, PageSource, SiteMetadata},
    renderer::RenderContext,
};

// TODO:
//
// This should get more robust. In particular, I'd like to be able to warn on
// something that looks like a source link but doesn't resolve to a file in the
// site. One challenge is that any link is technically valid, they just get
// passed through if we don't recognize it. This means we can only warn at best,
// since it will always be imperfect.
//
// One thing this will need to do it well is to plumb spans and locations from
// the markdown parser.

/// Finds links to source files and replaces them with links to the generated page
pub fn adjust_relative_links<'a>(
    markdown: impl Iterator<Item = Event<'a>>,
    page: &'a PageSource,
    rcx: &'a RenderContext<'_>,
) -> impl Iterator<Item = Event<'a>> {
    let map_url = |url: &CowStr<'_>| {
        let url = LinkDest::parse(url).ok()?;
        let anchor = url.fragment();
        if url.is_local() {
            debug!("found local link to {url}");
            let path = if url.is_relative() {
                page.source_path().parent()?.join(url.path())
            } else {
                rcx.site.root_dir().join(url.path())
            };
            debug!("mapped path to {}", path.display());
            let page = rcx.site.find_page_by_source_path(&path)?;
            let url = format!(
                "{}/{}{}",
                rcx.site.base_url(),
                page.url(),
                anchor.map(|a| format!("#{}", a)).unwrap_or_default()
            );
            debug!("linking to {url}");
            Some(url)
        } else {
            None
        }
    };

    markdown.map(move |event| match event {
        Event::Start(Tag::Link(link_type, url, title)) => {
            let url = map_url(&url).unwrap_or_else(|| url.to_string());
            Event::Start(Tag::Link(link_type, url.into(), title))
        }
        Event::End(Tag::Link(link_type, url, title)) => {
            let url = map_url(&url).unwrap_or_else(|| url.to_string());
            Event::End(Tag::Link(link_type, url.into(), title))
        }
        event => event,
    })
}

#[derive(Debug)]
enum LinkDest {
    External(Url),
    Local(String),
}

impl LinkDest {
    fn parse(s: &str) -> Result<Self, LinkDestError> {
        if let Ok(url) = Url::parse(s) {
            Ok(Self::External(url))
        } else {
            Ok(Self::Local(s.to_string()))
        }
    }

    fn is_local(&self) -> bool {
        match self {
            Self::External(_) => false,
            Self::Local(_) => true,
        }
    }

    fn is_relative(&self) -> bool {
        match self {
            Self::External(_) => false,
            Self::Local(s) => !s.starts_with('/'),
        }
    }

    fn fragment(&self) -> Option<&str> {
        match self {
            Self::External(url) => url.fragment(),
            Self::Local(s) => s.rsplit_once('#').map(|(_, f)| f),
        }
    }

    fn path(&self) -> &str {
        match self {
            Self::External(url) => url.path(),
            Self::Local(s) => s.split_once('#').map_or(s, |(p, _)| p),
        }
    }
}

impl std::fmt::Display for LinkDest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::External(url) => write!(f, "{}", url),
            Self::Local(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Diagnostic, Debug, Error)]
enum LinkDestError {}

#[cfg(test)]
mod test {
    use super::LinkDest;

    #[test]
    fn external_link() -> miette::Result<()> {
        let dest = LinkDest::parse("https://example.com")?;
        assert!(matches!(dest, LinkDest::External(_)));
        assert!(!dest.is_relative());
        Ok(())
    }

    #[test]
    fn local_link() -> miette::Result<()> {
        let dest = LinkDest::parse("/foo/bar")?;
        assert!(matches!(dest, LinkDest::Local(_)));
        assert!(!dest.is_relative());

        let dest = LinkDest::parse("foo/bar")?;
        assert!(matches!(dest, LinkDest::Local(_)));
        assert!(dest.is_relative());

        let dest = LinkDest::parse("../foo/bar")?;
        assert!(matches!(dest, LinkDest::Local(_)));
        assert!(dest.is_relative());

        Ok(())
    }

    #[test]
    fn fragment() -> miette::Result<()> {
        let dest = LinkDest::parse("https://example.com#foo")?;
        assert_eq!(dest.fragment(), Some("foo"));

        let dest = LinkDest::parse("/foo/bar#foo")?;
        assert_eq!(dest.fragment(), Some("foo"));

        let dest = LinkDest::parse("foo/bar#foo")?;
        assert_eq!(dest.fragment(), Some("foo"));

        let dest = LinkDest::parse("../foo/bar#foo")?;
        assert_eq!(dest.fragment(), Some("foo"));

        Ok(())
    }

    #[test]
    fn path() -> miette::Result<()> {
        let dest = LinkDest::parse("https://example.com")?;
        assert_eq!(dest.path(), "/");

        let dest = LinkDest::parse("/foo/bar")?;
        assert_eq!(dest.path(), "/foo/bar");

        let dest = LinkDest::parse("foo/bar")?;
        assert_eq!(dest.path(), "foo/bar");

        let dest = LinkDest::parse("../foo/bar")?;
        assert_eq!(dest.path(), "../foo/bar");

        Ok(())
    }
}

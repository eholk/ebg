use std::fmt::Formatter;

use email_address_parser::EmailAddress;
use miette::{diagnostic, Diagnostic};
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
    markdown: Vec<Event<'a>>,
    page: &PageSource,
    rcx: &RenderContext<'_>,
) -> Vec<Event<'a>> {
    let map_url = |url: &CowStr<'_>| {
        let url = LinkDest::parse(url).ok()?;
        let anchor = url.fragment();
        if url.is_possible_source_link() {
            debug!("found possible source link to {url}");
            let path = if url.is_relative() {
                let parent = page.source_path().parent()?;
                debug!("searching relative to `{}`", parent.display());
                parent.join(url.path())
            } else {
                rcx.site.root_dir().join(url.path())
            };
            debug!("mapped path to {}", path.display());
            let Some(page) = rcx.site.find_page_by_source_path(&path) else {
                debug!("no page found for {}", path.display());
                rcx.dcx.lock().unwrap().record(diagnostic!(
                    severity = miette::Severity::Warning,
                    help = "did you mean to link to an external page?",
                    "Could not find target for apparent source link to `{url}`",
                ));
                return None;
            };
            let url = format!(
                "/{}{}",
                // rcx.site.base_url(),
                page.url(),
                anchor.map(|a| format!("#{}", a)).unwrap_or_default()
            );
            debug!("linking to {url}");
            Some(url)
        } else {
            None
        }
    };

    markdown
        .into_iter()
        .map(move |event| match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let dest_url = map_url(&dest_url)
                    .unwrap_or_else(|| dest_url.to_string())
                    .into();
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
            event => event,
        })
        .collect()
}

#[derive(Debug)]
enum LinkDest {
    External(Url),
    Local(String),
    /// The link is an email address
    Email(String),
}

impl LinkDest {
    fn parse(s: &str) -> Result<Self, LinkDestError> {
        if let Ok(url) = Url::parse(s) {
            Ok(Self::External(url))
        } else if EmailAddress::parse(s, None).is_some() {
            Ok(Self::Email(s.to_string()))
        } else {
            Ok(Self::Local(s.to_string()))
        }
    }

    fn is_local(&self) -> bool {
        match self {
            Self::External(_) | Self::Email(_) => false,
            Self::Local(_) => true,
        }
    }

    fn is_relative(&self) -> bool {
        match self {
            Self::External(_) | Self::Email(_) => false,
            Self::Local(s) => !s.starts_with('/'),
        }
    }

    fn is_absolute(&self) -> bool {
        !self.is_relative()
    }

    fn fragment(&self) -> Option<&str> {
        match self {
            Self::External(url) => url.fragment(),
            Self::Local(s) => s.rsplit_once('#').map(|(_, f)| f),
            Self::Email(_) => None,
        }
    }

    fn path(&self) -> &str {
        match self {
            Self::External(url) => url.path(),
            Self::Local(s) => {
                let path = s.split_once('#').map_or(s.as_str(), |(p, _)| p);
                if path.starts_with("./") {
                    &path[2..]
                } else {
                    path
                }
            }
            Self::Email(source) => &source,
        }
    }

    /// Determines whether a reference could potentially be a link to a source
    /// file that's processed by EBG.
    fn is_possible_source_link(&self) -> bool {
        if !self.is_local() {
            return false;
        }
        if self.is_absolute() {
            return false;
        }

        let path = self.path();
        if path.is_empty() {
            return false;
        }
        if path.ends_with('/') {
            return false;
        }
        if path.ends_with(".md") {
            return true;
        }

        // err on the side of too many source links.
        true
    }
}

impl std::fmt::Display for LinkDest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::External(url) => write!(f, "{}", url),
            Self::Local(s) => write!(f, "{}", s),
            Self::Email(source) => write!(f, "{}", source),
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

        let dest = LinkDest::parse("./testimonials.md")?;
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

        let dest = LinkDest::parse("./testimonials.md")?;
        assert_eq!(dest.path(), "testimonials.md");

        Ok(())
    }

    #[test]
    fn is_possible_source_link() -> miette::Result<()> {
        let patterns = [
            ("https://example.com", false),
            ("./testimonials.md", true),
            ("#gat-desugaring", false),
            (
                "/blog/2013/09/10/how-to-write-a-simple-scheme-debugger/",
                false,
            ),
            ("/papers/dissertation.pdf", false),
            ("eric@theincredibleholk.org", false),
            ("/images/whereabouts-clock-drawing.pdf", false),
        ];

        for (pattern, expected) in patterns {
            let dest = LinkDest::parse(pattern)?;
            assert_eq!(dest.is_possible_source_link(), expected, "{}", pattern);
        }

        Ok(())
    }
}

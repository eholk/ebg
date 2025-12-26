//! Link parsing and classification utilities.

use std::fmt::Formatter;

use email_address_parser::EmailAddress;
use miette::Diagnostic;
use thiserror::Error;
use url::Url;

/// Represents a parsed link destination.
#[derive(Debug)]
pub enum LinkDest {
    /// An external URL with a scheme (http, https, etc.)
    External(Url),
    /// A local path (relative or absolute)
    Local(String),
    /// The link is an email address
    Email(String),
}

impl LinkDest {
    /// Parses a link destination string into a [`LinkDest`].
    ///
    /// This tries to parse as a URL first, then checks for email addresses,
    /// and finally treats everything else as a local path.
    pub fn parse(s: &str) -> Result<Self, LinkDestError> {
        if let Ok(url) = Url::parse(s) {
            Ok(Self::External(url))
        } else if EmailAddress::parse(s, None).is_some() {
            Ok(Self::Email(s.to_string()))
        } else {
            Ok(Self::Local(s.to_string()))
        }
    }

    /// Returns true if this is an external URL.
    pub fn is_external(&self) -> bool {
        matches!(self, Self::External(_))
    }

    /// Returns true if this is a local path (not external, not email).
    pub fn is_local(&self) -> bool {
        match self {
            Self::External(_) | Self::Email(_) => false,
            Self::Local(_) => true,
        }
    }

    /// Returns true if this is a relative path.
    ///
    /// Only local paths can be relative. External URLs and emails are never relative.
    pub fn is_relative(&self) -> bool {
        match self {
            Self::External(_) | Self::Email(_) => false,
            Self::Local(s) => !s.starts_with('/'),
        }
    }

    /// Returns true if this is an absolute path.
    pub fn is_absolute(&self) -> bool {
        !self.is_relative()
    }

    /// Extracts the fragment (anchor) from the link, if present.
    pub fn fragment(&self) -> Option<&str> {
        match self {
            Self::External(url) => url.fragment(),
            Self::Local(s) => s.rsplit_once('#').map(|(_, f)| f),
            Self::Email(_) => None,
        }
    }

    /// Returns the path component of the link.
    pub fn path(&self) -> &str {
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
            Self::Email(source) => source,
        }
    }

    /// Determines whether a reference could potentially be a link to a source
    /// file that's processed by EBG.
    ///
    /// This is used during rendering to identify links that should be rewritten
    /// to point to generated pages.
    pub fn is_possible_source_link(&self) -> bool {
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
pub enum LinkDestError {}

#[cfg(test)]
mod test {
    use super::*;

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

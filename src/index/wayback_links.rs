//! Per-post wayback link configuration.
//!
//! This module contains data structures for storing wayback machine archive
//! links on a per-post basis. Each post can have an associated `.wayback.toml`
//! file that tracks which external links have been archived and their
//! corresponding wayback URLs.

use chrono::{DateTime, Utc};
use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use url::Url;

/// Represents a single external link and its wayback machine archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaybackLink {
    /// The original URL that was archived.
    pub url: Url,
    /// The wayback machine URL for the archived version.
    pub wayback_url: Url,
    /// When this URL was archived.
    pub archived_at: DateTime<Utc>,
}

/// A collection of wayback links for a single post.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WaybackLinks {
    /// All the archived links for this post.
    #[serde(rename = "link", default)]
    links: Vec<WaybackLink>,
}

impl WaybackLinks {
    /// Creates a new empty collection of wayback links.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads wayback links from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WaybackLinksError> {
        let contents = std::fs::read_to_string(path.as_ref())
            .map_err(|e| WaybackLinksError::ReadFile(path.as_ref().to_path_buf(), e))?;

        toml::from_str(&contents).map_err(WaybackLinksError::ParseToml)
    }

    /// Writes wayback links to a TOML file.
    pub fn to_file(&self, path: impl AsRef<Path>) -> Result<(), WaybackLinksError> {
        let toml = toml::to_string_pretty(self).map_err(WaybackLinksError::SerializeToml)?;
        std::fs::write(path.as_ref(), toml)
            .map_err(|e| WaybackLinksError::WriteFile(path.as_ref().to_path_buf(), e))
    }

    /// Finds a wayback link for the given URL, if it exists.
    pub fn find(&self, url: &Url) -> Option<&WaybackLink> {
        self.links.iter().find(|link| &link.url == url)
    }

    /// Adds a wayback link to the collection.
    pub fn add(&mut self, link: WaybackLink) {
        self.links.push(link);
    }

    /// Returns true if the collection contains a wayback link for the given URL.
    pub fn contains(&self, url: &Url) -> bool {
        self.find(url).is_some()
    }

    /// Returns an iterator over all wayback links.
    pub fn iter(&self) -> impl Iterator<Item = &WaybackLink> {
        self.links.iter()
    }

    /// Returns the number of wayback links.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Returns true if there are no wayback links.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum WaybackLinksError {
    #[error("failed to read wayback links file {0}")]
    ReadFile(std::path::PathBuf, #[source] std::io::Error),

    #[error("failed to write wayback links file {0}")]
    WriteFile(std::path::PathBuf, #[source] std::io::Error),

    #[error("failed to parse TOML")]
    ParseToml(#[source] toml::de::Error),

    #[error("failed to serialize to TOML")]
    SerializeToml(#[source] toml::ser::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_empty() {
        let links = WaybackLinks::new();
        let toml = toml::to_string_pretty(&links).unwrap();
        assert_eq!(toml.trim(), "link = []");
    }

    #[test]
    fn test_deserialize_empty() {
        let links: WaybackLinks = toml::from_str("").unwrap();
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_serialize_single_link() {
        let mut links = WaybackLinks::new();
        links.add(WaybackLink {
            url: Url::parse("https://example.com/article").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240104034229/https://example.com/article",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-04T03:42:29Z")
                .unwrap()
                .into(),
        });

        let toml = toml::to_string_pretty(&links).unwrap();
        assert!(toml.contains("[[link]]"));
        assert!(toml.contains("url = \"https://example.com/article\""));
        assert!(toml.contains("wayback_url = \"https://web.archive.org/web/20240104034229/https://example.com/article\""));
        assert!(toml.contains("archived_at = \"2024-01-04T03:42:29Z\""));
    }

    #[test]
    fn test_deserialize_single_link() {
        let toml = r#"
[[link]]
url = "https://example.com/article"
wayback_url = "https://web.archive.org/web/20240104034229/https://example.com/article"
archived_at = "2024-01-04T03:42:29Z"
"#;

        let links: WaybackLinks = toml::from_str(toml).unwrap();
        assert_eq!(links.len(), 1);

        let link = links.iter().next().unwrap();
        assert_eq!(link.url.as_str(), "https://example.com/article");
        assert_eq!(
            link.wayback_url.as_str(),
            "https://web.archive.org/web/20240104034229/https://example.com/article"
        );
    }

    #[test]
    fn test_round_trip() {
        let mut original = WaybackLinks::new();
        original.add(WaybackLink {
            url: Url::parse("https://example.com/page1").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com/page1",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .into(),
        });
        original.add(WaybackLink {
            url: Url::parse("https://example.com/page2").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240102000000/https://example.com/page2",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
                .unwrap()
                .into(),
        });

        let toml = toml::to_string_pretty(&original).unwrap();
        let deserialized: WaybackLinks = toml::from_str(&toml).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_find() {
        let mut links = WaybackLinks::new();
        let url1 = Url::parse("https://example.com/page1").unwrap();
        let url2 = Url::parse("https://example.com/page2").unwrap();

        links.add(WaybackLink {
            url: url1.clone(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com/page1",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .into(),
        });

        assert!(links.find(&url1).is_some());
        assert!(links.find(&url2).is_none());
    }

    #[test]
    fn test_contains() {
        let mut links = WaybackLinks::new();
        let url1 = Url::parse("https://example.com/page1").unwrap();
        let url2 = Url::parse("https://example.com/page2").unwrap();

        links.add(WaybackLink {
            url: url1.clone(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com/page1",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .into(),
        });

        assert!(links.contains(&url1));
        assert!(!links.contains(&url2));
    }

    #[test]
    fn test_file_io() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.wayback.toml");

        // Create some links and write to file
        let mut original = WaybackLinks::new();
        original.add(WaybackLink {
            url: Url::parse("https://example.com/page1").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com/page1",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .into(),
        });

        original.to_file(&test_file).unwrap();

        // Read back from file
        let loaded = WaybackLinks::from_file(&test_file).unwrap();

        assert_eq!(original, loaded);

        // TempDir automatically cleans up on drop
    }

    #[test]
    fn test_print_example_toml() {
        let mut links = WaybackLinks::new();
        links.add(WaybackLink {
            url: Url::parse("https://example.com/article").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240104034229/https://example.com/article",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-04T03:42:29Z")
                .unwrap()
                .into(),
        });
        links.add(WaybackLink {
            url: Url::parse("https://another.com/page").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240105123456/https://another.com/page",
            )
            .unwrap(),
            archived_at: DateTime::parse_from_rfc3339("2024-01-05T12:34:56Z")
                .unwrap()
                .into(),
        });

        let toml = toml::to_string_pretty(&links).unwrap();
        println!("Example wayback.toml file:\n{}", toml);

        // Just verify it's parseable
        let parsed: Result<WaybackLinks, _> = toml::from_str(&toml);
        assert!(parsed.is_ok());
    }
}

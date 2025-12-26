use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

pub mod api;

pub use api::Wayback;

/// Describes information about a set of snapshots from for a site.
///
/// This is normally loaded from the `snapshots` file specified in `Site.toml`.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Snapshot {
    #[serde(flatten)]
    pages: HashMap<PathBuf, PageSnapshot>,
}

/// Describes information about a single page snapshot.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PageSnapshot {
    /// Maps a url to a key to be used in a wayback URL.
    #[serde(flatten)]
    external_links: HashMap<Url, String>,
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use miette::IntoDiagnostic;
    use url::Url;

    use super::Snapshot;

    #[test]
    fn parse_snapshots() -> miette::Result<()> {
        let snapshot_src = r#"
['_posts/2012-11-27-why-rust-is-awesome.md']
"https://www.rust-lang.org" = "20201127120000"
"#;
        let snapshot: super::Snapshot = toml::from_str(snapshot_src).into_diagnostic()?;

        assert_eq!(
            snapshot
                .pages
                .get(Path::new("_posts/2012-11-27-why-rust-is-awesome.md")),
            Some(&super::PageSnapshot {
                external_links: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        Url::parse("https://www.rust-lang.org").unwrap(),
                        "20201127120000".to_string(),
                    );
                    map
                }
            })
        );

        Ok(())
    }

    #[test]
    fn serialize_snapshots() -> miette::Result<()> {
        let snapshot = Snapshot {
            pages: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    Path::new("_posts/2012-11-27-why-rust-is-awesome.md").to_path_buf(),
                    super::PageSnapshot {
                        external_links: {
                            let mut map = std::collections::HashMap::new();
                            map.insert(
                                Url::parse("https://www.rust-lang.org").unwrap(),
                                "20201127120000".to_string(),
                            );
                            map
                        },
                    },
                );
                map
            },
        };

        let snapshot = toml::to_string(&snapshot).into_diagnostic()?;

        assert_eq!(
            snapshot,
            r#"["_posts/2012-11-27-why-rust-is-awesome.md"]
"https://www.rust-lang.org/" = "20201127120000"
"#
        );

        Ok(())
    }
}

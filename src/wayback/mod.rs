use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;
use url::Url;

pub mod api;

pub use api::Wayback;

/// Describes information about a set of snapshots from for a site.
///
/// This is normally loaded from the `snapshots` file specified in `Site.toml`.
#[derive(Deserialize, PartialEq, Debug)]
pub struct Snapshot {
    #[serde(flatten)]
    pages: HashMap<PathBuf, PageSnapshot>,
}

/// Describes information about a single page snapshot.
#[derive(Deserialize, PartialEq, Debug)]
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
}

use std::path::PathBuf;

use miette::Diagnostic;
use pulldown_cmark::{CowStr, Event, Tag};
use thiserror::Error;
use tracing::debug;
use url::Url;

use crate::{
    index::{PageMetadata, PageSource, SiteMetadata},
    renderer::RenderContext,
};

/// Finds links to source files and replaces them with links to the generated page
pub fn adjust_relative_links<'a>(
    markdown: impl Iterator<Item = Event<'a>>,
    page: &'a PageSource,
    rcx: &'a RenderContext<'_>,
) -> impl Iterator<Item = Event<'a>> {
    let map_url = |url: &CowStr<'_>| {
        let url = url.to_string();
        let (base, anchor) = match url.split_once('#') {
            Some((base, anchor)) => (base, Some(anchor)),
            None => (url.as_str(), None),
        };
        let path = PathBuf::from(&base);
        if path.is_relative() {
            debug!("found relative link to {}", path.display());
            let path = page.source_path().parent()?.join(path);
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

enum LinkDest {
    External(Url),
}

impl LinkDest {
    fn parse(s: &str) -> Result<Self, LinkDestError> {
        if let Ok(url) = Url::parse(s) {
            Ok(Self::External(url))
        } else {
            Err(LinkDestError::InvalidUrl(s.to_string()))
        }
    }
}

#[derive(Diagnostic, Debug, Error)]
enum LinkDestError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
}

#[cfg(test)]
mod test {
    use super::LinkDest;

    #[test]
    fn test_link_dest() -> miette::Result<()> {
        let dest = LinkDest::parse("https://example.com")?;
        assert!(matches!(dest, LinkDest::External(_)));
        Ok(())
    }
}

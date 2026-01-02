//! Adds wayback machine archive indicators to external links.
//!
//! This module processes markdown link events and adds visual indicators
//! (with links to the archived version) for external links that have been
//! archived in the Wayback Machine.

use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

use crate::index::{LinkDest, WaybackLinks};

/// Adds wayback machine indicators to archived external links.
///
/// For each external link that has a wayback archive, this adds a small
/// indicator link after the original link pointing to the archived version.
pub fn add_wayback_indicators<'a>(
    events: impl Iterator<Item = Event<'a>>,
    wayback_links: Option<&WaybackLinks>,
) -> impl Iterator<Item = Event<'a>> {
    // If there are no wayback links, just pass through
    let Some(wayback_links) = wayback_links else {
        return events.collect::<Vec<_>>().into_iter();
    };

    let mut output = Vec::new();
    let mut current_link_url: Option<String> = None;

    for event in events {
        match &event {
            Event::Start(Tag::Link {
                link_type: LinkType::Inline | LinkType::Reference | LinkType::Shortcut,
                dest_url,
                ..
            }) => {
                current_link_url = Some(dest_url.to_string());
                output.push(event);
            }
            Event::End(TagEnd::Link) => {
                output.push(event);

                // Check if this link should get a wayback indicator
                if let Some(url_str) = current_link_url.take() {
                    if let Ok(LinkDest::External(url)) = LinkDest::parse(&url_str) {
                        // Check if we have an archive for this URL
                        if let Some(wayback_link) = wayback_links.find(&url) {
                            // Add a space and then the archive indicator link
                            output.push(Event::Text(" ".into()));
                            output.push(Event::Start(Tag::Link {
                                link_type: LinkType::Inline,
                                dest_url: CowStr::from(wayback_link.wayback_url.to_string()),
                                title: CowStr::from(format!(
                                    "View archived version from {}",
                                    wayback_link.archived_at.format("%d %B %Y")
                                )),
                                id: CowStr::from(""),
                            }));
                            output.push(Event::Html(
                                "<span class=\"wayback-indicator\"></span>".into(),
                            ));
                            output.push(Event::End(TagEnd::Link));
                        }
                    }
                }
            }
            _ => {
                output.push(event);
            }
        }
    }

    output.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{WaybackLink, WaybackLinks};
    use chrono::Utc;
    use pulldown_cmark::{Parser, html};
    use url::Url;

    #[test]
    fn test_no_wayback_links() {
        let markdown = "Check out [this link](https://example.com)";
        let parser = Parser::new(markdown);
        let events: Vec<_> = add_wayback_indicators(parser, None).collect();

        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        assert!(html_output.contains("<a href=\"https://example.com\">this link</a>"));
        assert!(!html_output.contains("wayback"));
    }

    #[test]
    fn test_archived_link_gets_indicator() {
        let markdown = "Check out [this link](https://example.com)";

        let mut wayback_links = WaybackLinks::new();
        wayback_links.add(WaybackLink {
            url: Url::parse("https://example.com").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com",
            )
            .unwrap(),
            archived_at: Utc::now(),
        });

        let parser = Parser::new(markdown);
        let events: Vec<_> = add_wayback_indicators(parser, Some(&wayback_links)).collect();

        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        // Should have original link
        assert!(html_output.contains("<a href=\"https://example.com\">this link</a>"));
        // Should have wayback indicator
        assert!(html_output.contains("wayback-indicator"));
        assert!(html_output.contains("web.archive.org"));
    }

    #[test]
    fn test_unarchived_link_no_indicator() {
        let markdown =
            "Check out [this link](https://example.com) and [another](https://other.com)";

        let mut wayback_links = WaybackLinks::new();
        wayback_links.add(WaybackLink {
            url: Url::parse("https://example.com").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com",
            )
            .unwrap(),
            archived_at: Utc::now(),
        });

        let parser = Parser::new(markdown);
        let events: Vec<_> = add_wayback_indicators(parser, Some(&wayback_links)).collect();

        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        // First link should have indicator
        let first_link_pos = html_output.find("example.com").unwrap();
        let first_indicator_pos = html_output.find("wayback-indicator").unwrap();
        assert!(first_indicator_pos > first_link_pos);

        // Second link should not have indicator
        let second_link_pos = html_output.find("other.com").unwrap();
        let rest = &html_output[second_link_pos..];
        assert!(
            !rest.contains("wayback-indicator") || rest.find("wayback-indicator").unwrap() > 100
        );
    }

    #[test]
    fn test_local_links_ignored() {
        let markdown =
            "Check out [this local link](/blog/post) and [this external](https://example.com)";

        let mut wayback_links = WaybackLinks::new();
        wayback_links.add(WaybackLink {
            url: Url::parse("https://example.com").unwrap(),
            wayback_url: Url::parse(
                "https://web.archive.org/web/20240101000000/https://example.com",
            )
            .unwrap(),
            archived_at: Utc::now(),
        });

        let parser = Parser::new(markdown);
        let events: Vec<_> = add_wayback_indicators(parser, Some(&wayback_links)).collect();

        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        // Only one wayback indicator (for the external link)
        assert_eq!(html_output.matches("wayback-indicator").count(), 1);
    }
}

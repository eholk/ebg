//! Custom filters and other processors for the blog's markdown
//!
//! These are implemented as iterators from markdown events to markdown events.

use std::path::PathBuf;

use bumpalo::Bump;
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag};

mod code;
mod footnotes;

pub use code::CodeFormatter;
pub use footnotes::collect_footnotes;
use slug::slugify;
use tracing::debug;

use crate::index::{PageMetadata, PageSource, SiteMetadata};

use super::RenderContext;

/// Renders a page's markdown contents
///
/// If this is a new-style post (i.e. one that starts with an h1 that indicates the title), the
/// second field of the returned tuple will be the page's title extracted from the markdown
/// contents.
pub(super) fn render_markdown(
    source: &PageSource,
    rcx: &RenderContext<'_>,
) -> (String, Option<String>) {
    let contents = source.mainmatter();
    let parser = Parser::new_ext(
        contents,
        Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_HEADING_ATTRIBUTES,
    );

    let (parser, title) = extract_title_and_adjust_headers(parser);

    let parser = adjust_relative_links(parser, source, rcx);

    let mut anchors = HeadingAnchors::new();
    let parser = anchors.add_anchors(parser);

    let mut markdown_buffer = String::with_capacity(contents.len() * 2);
    pulldown_cmark::html::push_html(
        &mut markdown_buffer,
        rcx.code_formatter
            .format_codeblocks(collect_footnotes(parser)),
    );
    (markdown_buffer, title)
}

// pub fn trace_events<'a>(
//     parser: impl Iterator<Item = Event<'a>>,
// ) -> impl Iterator<Item = Event<'a>> {
//     parser.map(|e| {
//         trace!("{e:#?}");
//         e
//     })
// }

pub fn extract_title_and_adjust_headers<'a>(
    events: impl Iterator<Item = Event<'a>>,
) -> (impl Iterator<Item = Event<'a>>, Option<String>) {
    let mut output = vec![];

    enum State {
        Init,
        InTitle,
        PastTitle,
    }

    let mut state = State::Init;

    let mut has_title = false;
    let mut title = String::new();

    for event in events {
        match (&event, &state) {
            (Event::Start(Tag::Heading(HeadingLevel::H1, _fragment, _classes)), State::Init) => {
                state = State::InTitle;
                has_title = true;
            }
            (Event::End(Tag::Heading(HeadingLevel::H1, _fragment, _classes)), State::InTitle) => {
                state = State::PastTitle;
            }
            (_, State::Init) => {
                state = State::PastTitle;
                output.push(event);
            }
            (Event::Text(text) | Event::Html(text) | Event::Code(text), State::InTitle) => {
                title += text;
            }

            // Promote headings
            (Event::Start(Tag::Heading(level, fragment, classes)), State::PastTitle)
                if has_title =>
            {
                output.push(Event::Start(Tag::Heading(
                    promote_heading(*level),
                    *fragment,
                    classes.clone(),
                )))
            }
            (Event::End(Tag::Heading(level, fragment, classes)), State::PastTitle) if has_title => {
                output.push(Event::End(Tag::Heading(
                    promote_heading(*level),
                    *fragment,
                    classes.clone(),
                )))
            }

            (_, State::InTitle) => {}
            // FIXME: promote headings by one level when has_title is true
            (_, State::PastTitle) => output.push(event),
        }
    }

    (output.into_iter(), has_title.then_some(title))
}

fn promote_heading(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 | HeadingLevel::H2 => HeadingLevel::H1,
        HeadingLevel::H3 => HeadingLevel::H2,
        HeadingLevel::H4 => HeadingLevel::H3,
        HeadingLevel::H5 => HeadingLevel::H4,
        HeadingLevel::H6 => HeadingLevel::H5,
    }
}

pub struct HeadingAnchors {
    anchors: Bump,
}

impl HeadingAnchors {
    pub fn new() -> Self {
        Self {
            anchors: <_>::default(),
        }
    }

    pub fn add_anchors<'a, 'b>(
        &'a mut self,
        events: impl Iterator<Item = Event<'b>>,
    ) -> impl Iterator<Item = Event<'a>>
    where
        'b: 'a,
    {
        gen {
            let mut heading_text = String::new();

            let mut header_start = None;

            let mut out_events = Vec::with_capacity(match events.size_hint() {
                (min, max) => max.unwrap_or(min),
            });

            for mut event in events {
                match &mut event {
                    Event::Start(Tag::Heading(_level, None, _classes)) => {
                        heading_text = String::new();
                        header_start = Some(out_events.len());
                    }
                    Event::Text(text) if header_start.is_some() => heading_text += text,
                    Event::End(Tag::Heading(_level, end_fragment @ None, _classes))
                        if header_start.is_some() =>
                    {
                        let fragment = self.make_anchor(std::mem::take(&mut heading_text));

                        *end_fragment = Some(fragment);

                        match &mut out_events[header_start.unwrap()] {
                            Event::Start(Tag::Heading(_level, start_fragment @ None, _classes)) => {
                                *start_fragment = Some(fragment);
                            }
                            event => panic!("{event:?} is not a start header tag"),
                        }

                        header_start = None;

                        out_events.push(Event::Html(
                            format!("<a class=\"header-anchor\" href=\"#{fragment}\">🔗</a>")
                                .into(),
                        ));
                    }

                    _ => (),
                }

                out_events.push(event.clone());
                yield event
            }
        }
    }

    fn make_anchor(&self, text: impl AsRef<str>) -> &str {
        self.anchors.alloc_str(&heading_to_anchor(text.as_ref()))
    }
}

fn heading_to_anchor(heading: &str) -> String {
    slugify(heading)
}

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

#[cfg(test)]
mod test {
    use pulldown_cmark::{html::push_html, Event, HeadingLevel, Parser, Tag};

    use super::{extract_title_and_adjust_headers, heading_to_anchor};

    #[test]
    fn anchors() {
        assert_eq!(heading_to_anchor("Hello World"), "hello-world")
    }

    #[test]
    fn extract_title_heading() {
        let md = "
# This is the title

This is not
";

        let parser = Parser::new(md);

        let (_, title) = extract_title_and_adjust_headers(parser);

        assert_eq!(title, Some("This is the title".to_string()));
    }

    #[test]
    fn promote_titles() {
        let events = [
            Event::Start(Tag::Heading(HeadingLevel::H1, None, vec![])),
            Event::Text("This is the title".into()),
            Event::End(Tag::Heading(HeadingLevel::H1, None, vec![])),
            Event::Start(Tag::Heading(HeadingLevel::H2, None, vec![])),
            Event::Text("This is a section".into()),
            Event::End(Tag::Heading(HeadingLevel::H2, None, vec![])),
        ];

        let (events, title) = extract_title_and_adjust_headers(events.into_iter());

        assert_eq!(
            events.collect::<Vec<_>>(),
            vec![
                Event::Start(Tag::Heading(HeadingLevel::H1, None, vec![])),
                Event::Text("This is a section".into()),
                Event::End(Tag::Heading(HeadingLevel::H1, None, vec![])),
            ]
        );
        assert_eq!(title, Some("This is the title".to_string()));
    }

    #[test]
    fn add_anchors() {
        let mut anchors = super::HeadingAnchors::new();
        let events = Parser::new(
            "# This is the title

this is not the title

## This is a section
",
        );
        let events = anchors.add_anchors(events);

        let mut html = String::new();
        push_html(&mut html, events);

        assert!(html.contains("<a class=\"header-anchor\" href=\"#this-is-the-title\">🔗</a>"));
        assert!(html.contains("<a class=\"header-anchor\" href=\"#this-is-a-section\">🔗</a>"));
    }
}

//! Custom filters and other processors for the blog's markdown
//!
//! These are implemented as iterators from markdown events to markdown events.

use self::source_links::LinkDest;

use super::RenderContext;
use crate::index::{PageSource, SiteMetadata, WaybackConfig};
use bumpalo::Bump;
use pulldown_cmark::{CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag};
use slug::slugify;

mod code;
mod footnotes;
mod source_links;

pub use code::CodeFormatter;
pub use footnotes::collect_footnotes;
pub use source_links::adjust_relative_links;

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

    let parser = adjust_relative_links(parser.collect(), source, rcx);

    let mut anchors = HeadingAnchors::new();
    let parser = anchors.add_anchors(parser.into_iter());

    let parser = collect_footnotes(parser);
    let parser = rcx.code_formatter.format_codeblocks(parser);

    let parser: Vec<_> = match &rcx.site.config().wayback {
        Some(wayback) => add_wayback_links(wayback, parser.into_iter()).collect(),
        None => parser.collect(),
    };

    let mut markdown_buffer = String::with_capacity(contents.len() * 2);
    pulldown_cmark::html::push_html(&mut markdown_buffer, parser.into_iter());
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

/// [`HeadingAnchors`] is a processor that adds anchors to headings if they have
/// not been manually specified.
///
/// Additionally, it will add a convenience 🔗 link at the end to go to the
/// anchor.
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
                        format!("<a class=\"header-anchor\" href=\"#{fragment}\">🔗</a>").into(),
                    ));
                }

                _ => (),
            }

            out_events.push(event)
        }

        out_events.into_iter()
    }

    fn make_anchor(&self, text: impl AsRef<str>) -> &str {
        self.anchors.alloc_str(&heading_to_anchor(text.as_ref()))
    }
}

fn heading_to_anchor(heading: &str) -> String {
    slugify(heading)
}

pub fn add_wayback_links<'a>(
    config: &WaybackConfig,
    events: impl Iterator<Item = Event<'a>>,
) -> impl Iterator<Item = Event<'a>> {
    let mut count = 0;
    let mut footnotes = Vec::new();
    let events: Vec<_> = events
        .flat_map(|e| match e {
            Event::End(Tag::Link(ref _ty, ref url, ref _title)) => {
                let Ok(dest) = LinkDest::parse(url) else {
                    return vec![e];
                };

                if dest.is_external() {
                    let tag: CowStr<'_> = format!("wayback-{count}").into();

                    // FIXME: derive the timestamp either from wayback.toml or the page's publish date
                    let timestamp = "20240108";

                    footnotes.push(Event::Start(Tag::FootnoteDefinition(tag.clone())));
                    footnotes.push(Event::Text("Wayback: ".into()));
                    footnotes.push(Event::Start(Tag::Link(
                        LinkType::Autolink,
                        format!("https://web.archive.org/web/{timestamp}/{url}",).into(),
                        "".into(),
                    )));
                    footnotes.push(Event::Text(url.clone()));
                    footnotes.push(Event::End(Tag::Link(
                        LinkType::Autolink,
                        format!("https://web.archive.org/web/{timestamp}/{url}",).into(),
                        "".into(),
                    )));
                    footnotes.push(Event::End(Tag::FootnoteDefinition(tag.clone())));

                    count += 1;
                    vec![e, Event::FootnoteReference(tag.clone())]
                } else {
                    vec![e]
                }
            }
            e => vec![e],
        })
        .collect();

    events.into_iter().chain(footnotes.into_iter())
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

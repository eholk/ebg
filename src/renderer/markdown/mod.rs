//! Custom filters and other processors for the blog's markdown
//!
//! These are implemented as iterators from markdown events to markdown events.

use self::anchors::HeadingAnchors;

use super::RenderContext;
use crate::index::PageSource;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

mod anchors;
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
            (
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H1,
                    ..
                }),
                State::Init,
            ) => {
                state = State::InTitle;
                has_title = true;
            }
            (Event::End(TagEnd::Heading(HeadingLevel::H1)), State::InTitle) => {
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
            (
                Event::Start(Tag::Heading {
                    level,
                    id: fragment,
                    classes,
                    attrs,
                }),
                State::PastTitle,
            ) if has_title => output.push(Event::Start(Tag::Heading {
                level: promote_heading(*level),
                id: fragment.clone(),
                classes: classes.clone(),
                attrs: attrs.clone(),
            })),
            (Event::End(TagEnd::Heading(level)), State::PastTitle) if has_title => {
                output.push(Event::End(TagEnd::Heading(promote_heading(*level))))
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

#[cfg(test)]
mod test {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    use super::{extract_title_and_adjust_headers};

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
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                id: None,
                classes: vec![],
                attrs: vec![],
            }),
            Event::Text("This is the title".into()),
            Event::End(TagEnd::Heading(HeadingLevel::H1)),
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                id: None,
                classes: vec![],
                attrs: vec![],
            }),
            Event::Text("This is a section".into()),
            Event::End(TagEnd::Heading(HeadingLevel::H2)),
        ];

        let (events, title) = extract_title_and_adjust_headers(events.into_iter());

        assert_eq!(
            events.collect::<Vec<_>>(),
            vec![
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H1,
                    id: None,
                    classes: vec![],
                    attrs: vec![],
                }),
                Event::Text("This is a section".into()),
                Event::End(TagEnd::Heading(HeadingLevel::H1)),
            ]
        );
        assert_eq!(title, Some("This is the title".to_string()));
    }
}

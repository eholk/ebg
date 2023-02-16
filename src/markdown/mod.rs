//! Custom filters and other processors for the blog's markdown
//!
//! These are implemented as iterators from markdown events to markdown events.

use pulldown_cmark::{Event, HeadingLevel, Tag};

mod code;

pub use code::CodeFormatter;

/// Gathers all footnote definitions and pulls them to the end
pub fn collect_footnotes<'a>(
    parser: impl Iterator<Item = Event<'a>>,
) -> impl Iterator<Item = Event<'a>> {
    CollectFootnotes::Parsing {
        parser,
        footnotes: vec![],
        in_footnote: false,
    }
}

enum CollectFootnotes<'a, I> {
    Parsing {
        parser: I,
        footnotes: Vec<Event<'a>>,
        in_footnote: bool,
    },
    Finishing {
        footnotes: std::vec::IntoIter<Event<'a>>,
    },
}

impl<'a, I> Iterator for CollectFootnotes<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self {
                CollectFootnotes::Parsing {
                    parser,
                    footnotes,
                    in_footnote,
                } => match parser.next() {
                    Some(e) => match e {
                        Event::Start(Tag::FootnoteDefinition(tag)) => {
                            *in_footnote = true;
                            footnotes.push(Event::Start(Tag::FootnoteDefinition(tag)));
                        }
                        Event::End(Tag::FootnoteDefinition(tag)) => {
                            *in_footnote = false;
                            footnotes.push(Event::End(Tag::FootnoteDefinition(tag)));
                        }
                        e => {
                            if *in_footnote {
                                footnotes.push(e)
                            } else {
                                return Some(e);
                            }
                        }
                    },
                    None => {
                        *self = Self::Finishing {
                            footnotes: std::mem::take(footnotes).into_iter(),
                        };
                        return Some(Event::Rule);
                    }
                },
                CollectFootnotes::Finishing { footnotes } => return footnotes.next(),
            }
        }
    }
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

#[cfg(test)]
mod test {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

    use super::extract_title_and_adjust_headers;

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
}

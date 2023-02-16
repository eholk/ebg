//! Markdown filters for adjusting the way footnotes show up.

use pulldown_cmark::{Event, Tag};

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

//! Markdown filters for adjusting the way footnotes show up.

use pulldown_cmark::{Event, Tag};
use tracing::debug;

/// Gathers all footnote definitions and pulls them to the end
pub fn collect_footnotes<'a>(
    parser: impl Iterator<Item = Event<'a>>,
) -> impl Iterator<Item = Event<'a>> {
    CollectFootnotes::Parsing {
        parser,
        footnotes: vec![],
        in_footnote: false,
        count: 0,
    }
}

enum CollectFootnotes<'a, I> {
    Parsing {
        parser: I,
        footnotes: Vec<Event<'a>>,
        in_footnote: bool,
        /// How many footnotes we've encountered so far.
        count: usize,
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
                    count,
                } => {
                    match parser.next() {
                        Some(e) => {
                            match e {
                                Event::FootnoteReference(tag) => {
                                    // Manually render footnote here so we can add a backlink id
                                    *count += 1;
                                    let html = format!(
                                        r##"<sup class="footnote-reference"><a href="#{tag}" id="fnref:{tag}">{count}</a></sup>"##,
                                    );
                                    return Some(Event::Html(html.into()));
                                }
                                Event::Start(Tag::FootnoteDefinition(tag)) => {
                                    *in_footnote = true;
                                    footnotes.push(Event::Start(Tag::FootnoteDefinition(tag)));
                                }
                                Event::End(Tag::FootnoteDefinition(tag)) => {
                                    *in_footnote = false;
                                    debug!("ending footnote, last event = {:?}", footnotes.last());
                                    assert_eq!(footnotes.last(), Some(&Event::End(Tag::Paragraph)));
                                    footnotes.insert(footnotes.len() - 1, Event::Html(format!(r##"<a href="#fnref:{tag}" class="footnote-backref">↩</a>"##).into()));
                                    footnotes.push(Event::End(Tag::FootnoteDefinition(tag)));
                                }
                                e => {
                                    if *in_footnote {
                                        footnotes.push(e)
                                    } else {
                                        return Some(e);
                                    }
                                }
                            }
                        }
                        None => {
                            *self = Self::Finishing {
                                footnotes: std::mem::take(footnotes).into_iter(),
                            };
                            return Some(Event::Rule);
                        }
                    }
                }
                CollectFootnotes::Finishing { footnotes } => return footnotes.next(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser};

    #[test]
    fn test_collect_footnotes() {
        let input = r##"
This is a footnote[^1].

[^1]: this is the footnote text

The footnote should come after this.
"##;
        let events = Parser::new_ext(input, Options::ENABLE_FOOTNOTES);
        let events = collect_footnotes(events);
        assert!(matches!(
            events.last(),
            Some(Event::End(Tag::FootnoteDefinition(_)))
        ));
    }
}

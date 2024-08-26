//! Markdown filters for adjusting the way footnotes show up.

use pulldown_cmark::{Event, Tag, TagEnd};

/// Gathers all footnote definitions and pulls them to the end
pub gen fn collect_footnotes<'a>(mut parser: impl Iterator<Item = Event<'a>>) -> Event<'a> {
    let mut count = 0;
    let mut footnotes = vec![];

    // can't use a for loop here because it would lead to a borrow across yield
    while let Some(e) = parser.next() {
        match e {
            Event::FootnoteReference(tag) => {
                count += 1;
                // Manually render footnote here so we can add a backlink id
                let html = format!(
                    r##"<sup class="footnote-reference"><a href="#{tag}" id="fnref:{tag}">{count}</a></sup>"##,
                );
                yield Event::Html(html.into());
            }
            Event::Start(Tag::FootnoteDefinition(tag)) => {
                footnotes.push(Event::Start(Tag::FootnoteDefinition(tag.clone())));
                collect_footnote_def(&mut parser, &tag, &mut footnotes);
            }
            e => {
                yield e;
            }
        }
    }

    if !footnotes.is_empty() {
        yield Event::Rule;
    }

    for e in footnotes {
        yield e;
    }
}

fn collect_footnote_def<'a>(
    parser: impl Iterator<Item = Event<'a>>,
    tag: &str,
    footnotes: &mut Vec<Event<'a>>,
) {
    for e in parser {
        match e {
            Event::End(TagEnd::FootnoteDefinition) => {
                assert_eq!(footnotes.last(), Some(&Event::End(TagEnd::Paragraph)));
                footnotes.insert(
                    footnotes.len() - 1,
                    Event::Html(
                        format!(r##"<a href="#fnref:{tag}" class="footnote-backref">↩</a>"##)
                            .into(),
                    ),
                );
                footnotes.push(e);
                return;
            }
            Event::Start(Tag::FootnoteDefinition(_)) => {
                panic!("nested footnotes not supported");
            }
            e => footnotes.push(e),
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
            Some(Event::End(TagEnd::FootnoteDefinition))
        ));
    }
}

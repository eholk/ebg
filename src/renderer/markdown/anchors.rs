use bumpalo::Bump;
use pulldown_cmark::{Event, Tag, TagEnd};
use slug::slugify;

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
                Event::Start(Tag::Heading { id: None, .. }) => {
                    heading_text = String::new();
                    header_start = Some(out_events.len());
                }
                Event::Text(text) | Event::Code(text) if header_start.is_some() => {
                    heading_text += text
                }
                Event::End(TagEnd::Heading(_)) if header_start.is_some() => {
                    let fragment = self.make_anchor(std::mem::take(&mut heading_text));

                    match &mut out_events[header_start.unwrap()] {
                        Event::Start(Tag::Heading {
                            id: start_fragment @ None,
                            ..
                        }) => {
                            *start_fragment = Some(fragment.into());
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

#[cfg(test)]
mod test {
    use super::heading_to_anchor;
    use pulldown_cmark::{html::push_html, Event, Parser, Tag};

    /// Makes sure we generate the right anchor for various headers
    #[test]
    fn anchors() {
        assert_eq!(heading_to_anchor("Hello World"), "hello-world")
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

    /// Makes sure we generate something when anchors have code snippets in them
    ///
    /// Regression test for #75
    #[test]
    fn code_anchor() {
        let mut anchors = super::HeadingAnchors::new();
        let events = Parser::new("# `this is a code snippet`");
        let events: Vec<_> = anchors.add_anchors(events).collect();
        assert!(events.contains(&Event::Start(Tag::Heading {
            id: Some("this-is-a-code-snippet".into()),
            level: pulldown_cmark::HeadingLevel::H1,
            classes: vec![],
            attrs: vec![],
        })))
    }

    /// Makes sure we generate something when anchors have code snippets and regular text
    ///
    /// Regression test for #75
    #[test]
    fn mixed_code_anchor() {
        let mut anchors = super::HeadingAnchors::new();
        let events = Parser::new("# Heading with `code snippets`");
        let events: Vec<_> = anchors.add_anchors(events).collect();
        assert!(events.contains(&Event::Start(Tag::Heading {
            id: Some("heading-with-code-snippets".into()),
            level: pulldown_cmark::HeadingLevel::H1,
            classes: vec![],
            attrs: vec![],
        })))
    }
}

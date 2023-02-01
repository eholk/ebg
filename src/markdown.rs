//! Custom filters and other processors for the blog's markdown
//!
//! These are implemented as iterators from markdown events to markdown events.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag};
use std::collections::HashMap;
use syntect::{highlighting::ThemeSet, html::highlighted_html_for_string, parsing::SyntaxSet};

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

pub struct CodeFormatter {
    /// Maps language names that would show up in a code block header to a file extension that can
    /// be used to select a syntax set.
    language_map: HashMap<&'static str, &'static str>,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl CodeFormatter {
    pub fn new() -> Self {
        Self {
            language_map: [("rust", "rs")].into(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    fn highlight_code(&self, code: String, lang: LangOptions<'_>) -> Vec<Event<'_>> {
        let lines: Option<usize> = lang.line_numbers.then(|| code.lines().map(|_| 1).sum());

        let syntax = lang.lang.and_then(|lang| {
            let extension = self.language_map.get(lang).unwrap_or(&lang);
            self.syntax_set.find_syntax_by_extension(extension)
        });

        let body = match syntax {
            Some(ss) => {
                vec![Event::Html(
                    highlighted_html_for_string(
                        &code,
                        &self.syntax_set,
                        ss,
                        &self.theme_set.themes["InspiredGitHub"],
                    )
                    .unwrap()
                    .into(),
                )]
            }
            None => vec![
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                    lang.lang.unwrap_or("").to_owned().into(),
                ))),
                Event::Text(code.into()),
                Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(
                    lang.lang.unwrap_or("").to_owned().into(),
                ))),
            ],
        };

        match lines {
            Some(count) => {
                let mut events = vec![
                    Event::Html("<table class=\"codenum\"><tbody><tr><td>".into()),
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced("".into()))),
                    Event::Text(
                        (1..(count + 1))
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join("\n")
                            .into(),
                    ),
                    Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(
                        lang.lang().to_owned().into(),
                    ))),
                    Event::Html("</td><td>".into()),
                ];
                events.extend(body);
                events.push(Event::Html("</td></tr></tbody></table>".into()));
                events
            }
            None => body,
        }
    }

    pub fn format_codeblocks<'a>(
        &'a self,
        parser: impl Iterator<Item = Event<'a>>,
    ) -> impl Iterator<Item = Event<'a>> {
        let mut in_code = false;
        let mut code = String::new();
        parser
            .flat_map(|e| match e {
                Event::Start(Tag::CodeBlock(_lang)) => {
                    in_code = true;
                    vec![]
                }
                Event::End(Tag::CodeBlock(lang)) => {
                    in_code = false;
                    let code = std::mem::take(&mut code);
                    match lang {
                        CodeBlockKind::Fenced(lang) => {
                            let lang = parse_lang(lang.as_ref());
                            self.highlight_code(code, lang)
                        }
                        CodeBlockKind::Indented => vec![
                            Event::Start(Tag::CodeBlock(lang.clone())),
                            Event::Text(code.into()),
                            Event::End(Tag::CodeBlock(lang)),
                        ],
                    }
                }
                Event::Text(text) => {
                    if in_code {
                        code += text.as_ref();
                        vec![]
                    } else {
                        vec![Event::Text(text)]
                    }
                }
                e => vec![e],
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl Default for CodeFormatter {
    fn default() -> Self {
        Self::new()
    }
}

struct LangOptions<'a> {
    lang: Option<&'a str>,
    line_numbers: bool,
}

impl LangOptions<'_> {
    fn lang(&self) -> &str {
        self.lang.unwrap_or("")
    }
}

fn parse_lang(s: &str) -> LangOptions<'_> {
    let line_numbers = s.ends_with('=');
    let lang = s.rsplit_once('=').map(|(lang, _)| lang).unwrap_or(s);
    let lang = (!lang.is_empty()).then_some(lang);
    LangOptions { lang, line_numbers }
}

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

    use super::{extract_title_and_adjust_headers, parse_lang};

    #[test]
    fn parse_lang_options() -> eyre::Result<()> {
        let opts = parse_lang("rust=");
        assert_eq!(opts.lang, Some("rust"));
        assert!(opts.line_numbers);

        let opts = parse_lang("rust");
        assert_eq!(opts.lang, Some("rust"));
        assert!(!opts.line_numbers);

        let opts = parse_lang("");
        assert_eq!(opts.lang, None);
        assert!(!opts.line_numbers);

        let opts = parse_lang("=");
        assert_eq!(opts.lang, None);
        assert!(opts.line_numbers);

        Ok(())
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
}

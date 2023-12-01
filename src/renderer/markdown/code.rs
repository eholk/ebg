//! Markdown filters for syntax highlighting and other code formatting.

use pulldown_cmark::{CodeBlockKind, Event, Tag};
use std::collections::HashMap;
use syntect::{highlighting::ThemeSet, html::highlighted_html_for_string, parsing::SyntaxSet};

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

    fn highlight_code(&self, code: String, lang: LangOptions<'_>) -> impl Iterator<Item = Event<'_>> {
        let lines: Option<usize> = lang.line_numbers.then(|| code.lines().map(|_| 1).sum());

        let syntax = lang.lang.and_then(|lang| {
            let extension = self.language_map.get(lang).unwrap_or(&lang);
            self.syntax_set.find_syntax_by_extension(extension)
        });

        let lang = lang.lang().to_owned();
        let lang_clone = lang.clone();
        let body = gen move {
            match syntax {
                Some(ss) => {
                    yield Event::Html(
                        highlighted_html_for_string(
                            &code,
                            &self.syntax_set,
                            ss,
                            &self.theme_set.themes["InspiredGitHub"],
                        )
                        .unwrap()
                        .into(),
                    );
                }
                None => {
                    yield Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                        lang.clone().into(),
                    )));
                    yield Event::Text(code.into());
                    yield Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(
                        lang.into(),
                    )));
                },
            }
        };

        let lang = lang_clone;
        gen move {
            match lines {
                Some(count) => {
                    yield Event::Html("<table class=\"codenum\"><tbody><tr><td>".into());
                    yield Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced("".into())));
                    yield Event::Text(
                            (1..(count + 1))
                                .map(|i| i.to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                                .into(),
                        );
                    yield Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(
                            lang.into(),
                        )));
                    yield Event::Html("</td><td>".into());
                    for e in body {
                        yield e;
                    }
                    yield Event::Html("</td></tr></tbody></table>".into());
                }
                None => for e in body {
                    yield e;
                },
            }
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
                            self.highlight_code(code, lang).collect()
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

#[derive(Clone)]
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

#[cfg(test)]
mod test {
    use crate::renderer::markdown::code::parse_lang;

    #[test]
    fn parse_lang_options() -> miette::Result<()> {
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
}

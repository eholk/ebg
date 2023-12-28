---
layout: page
---

# Eric's Blog Generator

This site describes how to use Eric's Blog Generator (EBG).
It also serves as an example site generated using Eric's Blog Generator.

## Getting Started - Configuration

EBG expects each page to have a configuration file called `Site.toml`.
This file provides basic information, like what files to process.

Here is an example:

```toml
title = "My Awesome Web Site"

content = [
    "images",
    "index.md",
    "style.css",
]
```

For more detail about `Site.toml`, see [Site Configuration](site-config.md).

## Anatomy of a Page and Post

Pages may be written as HTML or Markdown.
Additional information is specified in YAML frontmatter.
A simple HTML page would look something like this:

```html
---
title: Sample Page
---

<p>This is a sample page.</p>
```

The lines between the `---` are the frontmatter.
The remainder of the post comes afterwards.

For Markdown files, these are converted to HTML, while HTML content is rendered unchanged into the site template.
After rendering, site macros are expanded.

### Markdown

Markdown files are converted to HTML using [Pulldown][pulldown].
Some extensions are enabled by default, such as footnotes.[^exfootnote]

[pulldown]: https://crates.io/crates/pulldown-cmark

[^exfootnote]: Footnotes are rendered like this.

## Themes

See [Themes](themes.md).

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

## Templates

Templates describe how page content should be formatted.
EBG uses the [Tera Templating Engine][tera] and expects templates to be found in the `templates` directory relative to `Site.toml`.

[tera]: https://tera.netlify.app/

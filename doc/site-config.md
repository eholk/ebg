---
layout: page
---

# Site Configuration

Sites in EBG are configured using a TOML file called `Site.toml`.
This page describes the various supported configuration options.

## Top Level Configuration

- `title`: The title of the site.
  This is available to themes as `site.title`.
- `subtitle`: A tagline or subtitle for the site.
  This is available to themes as `site.subtitle`.
- `author`: The name of the author of the site.
  This is available to themes as `site.author`.
- `author_email`: The email address of the author of the site.
  This is available to themes as `site.author_email`.
- `url`: The base URL for where the site is ultimately published. Most generated
  links will be prefixed by this.
  This is available to themes as `site.url`.
- `posts`: The directory containing posts. Posts are handled specially because
  their filename is parsed to extract details like the publication date. The
  publication date is also used to generate a friendly link.
- `content`: A list of files and directories to process. EBG will not process
  files that are not included in this list.
- `theme`: The name of the theme to use. This is the name of a directory
  relative to `Site.toml` that includes Tera templates that are used to generate
  the site. See [Themes](themes.md) for more information.
- `theme_opts`: This section is passed to the theme under the `theme` variable.
  It's used to set theme-specific options, such as a list of top-level
  navigation links. See the documentation for your theme to see what options are available.
- `macros`: A list of macros to make available to the site. They are typically
  listed as `m = macros.html`, and then the macros defined in `macros.html` are
  available under the `m::` namespace. See the [Tera Macros page][tera-macros]
  for more information.
- `wayback`: Configuration for automatic external link archiving. See
  [Wayback](wayback.md) for more information.

[tera-macros]: https://keats.github.io/tera/docs/#macros

---
layout: page
---

# Themes

Themes describe how page content should be formatted.
EBG uses the [Tera Templating Engine][tera] and expects templates to be found in the `theme` directory relative to `Site.toml`.
This location can be overriden using the theme property in `Site.toml`.[^theme]
For example:

```toml
theme = "./themes/my_awesome_theme/"
```

[tera]: https://tera.netlify.app/

[^theme]: Although this feature isn't used much, in theory this would make it easy to switch themes for EBG sites.

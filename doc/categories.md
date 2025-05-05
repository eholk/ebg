---
layout: page
title: Categories
---

# Category Pages

EBG supports categorizing your posts and generating category-specific index pages. This feature allows readers to browse posts by topic.

## Adding Categories to Posts

To add categories to a post, include them in the frontmatter:

```yaml
---
title: My Post About Rust
categories:
  - Programming Languages
  - Rust
---
```

## Generating Category Pages

Category pages are automatically generated if your theme includes a `category.html` template. They will be created at `/blog/category/[category-slug]/`.

For example, posts in the "Programming Languages" category will be listed at `/blog/category/programming-languages/`.

## Creating a Category Template

To enable category pages, add a `category.html` template to your theme. This template will have access to:

- `category`: The name of the current category
- `posts`: An array of posts in this category
- `site`: The global site data

Here's a basic example of a category template:

```html
<!DOCTYPE html>
<html>
<head>
    <title>Category: title</title>
</head>
<body>
    <h1>Category: name</h1>
    
    <div class="posts">
        <!-- Loop through posts -->
        <article>
            <h2><a href="/post-url/">Post Title</a></h2>
            <time>January 1, 2023</time>
            <div>Post excerpt here...</div>
        </article>
        <!-- End loop -->
    </div>
</body>
</html>
```

Posts are automatically sorted by date (newest first).

## Linking to Category Pages

You can create links to your category pages in your templates:

```html
<ul class="categories">
    <!-- Loop through categories -->
    <li>
        <a href="/blog/category/category-slug/">
            Category Name (5)
        </a>
    </li>
    <!-- End loop -->
</ul>
```

## Note on Slugification

Category URLs are created by converting the category name to a slug. For example:
- "Programming Languages" becomes "programming-languages" 
- "C++" becomes "c"
- "Web 3.0" becomes "web-3-0"

Use the same slugification in your templates when creating links to category pages.

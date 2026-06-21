//! Adjusts relative links in markdown to point to the correct URLs
//!
//! This handles both page links (e.g., `[text](./other-page.md)`) and image sources
//! (e.g., `![alt](../images/image.png)`), converting them to absolute URLs.

use pulldown_cmark::{Event, Tag, TagEnd, CowStr};
use crate::index::PageSource;
use super::RenderContext;
use std::path::{Path, PathBuf};

/// Adjusts relative links and image sources in markdown events
///
/// This converts relative paths like `./other-page.md` or `../images/image.png`
/// to their corresponding absolute URLs in the generated site.
pub fn adjust_relative_links<'a>(
    events: Vec<Event<'a>>,
    source: &'a PageSource,
    rcx: &RenderContext<'_>,
) -> Vec<Event<'a>> {
    events
        .into_iter()
        .map(|event| adjust_event(event, source, rcx))
        .collect()
}

fn adjust_event<'a>(
    event: Event<'a>,
    source: &'a PageSource,
    rcx: &RenderContext<'_>,
) -> Event<'a> {
    match event {
        Event::Start(Tag::Link { dest, title, id }) => {
            let adjusted_dest = adjust_link(&dest, source, rcx);
            Event::Start(Tag::Link {
                dest: adjusted_dest,
                title,
                id,
            })
        }
        Event::Start(Tag::Image { dest, title, id }) => {
            let adjusted_dest = adjust_link(&dest, source, rcx);
            Event::Start(Tag::Image {
                dest: adjusted_dest,
                title,
                id,
            })
        }
        other => other,
    }
}

fn adjust_link(dest: &str, source: &PageSource, rcx: &RenderContext<'_>) -> CowStr {
    // Don't adjust absolute URLs, anchors, or external links
    if dest.starts_with('#') || dest.starts_with("http://") || dest.starts_with("https://") {
        return CowStr::Borrowed(dest);
    }

    // Try to resolve as a page source link first
    if let Some(resolved) = try_resolve_page_link(dest, source, rcx) {
        return CowStr::Boxed(resolved.into_boxed_str());
    }

    // If not a page link, treat as a relative file path (e.g., image)
    if let Some(resolved) = try_resolve_file_link(dest, source) {
        return CowStr::Boxed(resolved.into_boxed_str());
    }

    // If resolution fails, return the original
    CowStr::Borrowed(dest)
}

/// Try to resolve a link as a page source (e.g., `./2012-10-14-hello.md`)
fn try_resolve_page_link(
    dest: &str,
    source: &PageSource,
    rcx: &RenderContext<'_>,
) -> Option<String> {
    // Extract the path part (before any #anchor)
    let (path_part, anchor) = dest.split_once('#').unwrap_or((dest, ""));

    // Only process if it looks like a markdown file
    if !path_part.ends_with(".md") {
        return None;
    }

    // Resolve the relative path
    let source_dir = Path::new(source.source_path()).parent()?;
    let target_path = source_dir.join(path_part);
    let normalized = normalize_path(&target_path);

    // Try to find the page in the site index
    let target_page = rcx.site.find_page_by_source_path(&normalized)?;
    let url = target_page.url();

    // Reconstruct with anchor if present
    let result = if anchor.is_empty() {
        format!("{}/", url)
    } else {
        format!("{}/#{}" , url, anchor)
    };

    Some(result)
}

/// Try to resolve a link as a relative file path (e.g., `../images/image.png`)
fn try_resolve_file_link(dest: &str, source: &PageSource) -> Option<String> {
    // Extract the path part (before any #anchor)
    let (path_part, anchor) = dest.split_once('#').unwrap_or((dest, ""));

    // Don't process if it's a markdown file (those should be handled by try_resolve_page_link)
    if path_part.ends_with(".md") {
        return None;
    }

    // Resolve the relative path from the source file's directory
    let source_dir = Path::new(source.source_path()).parent()?;
    let target_path = source_dir.join(path_part);
    let normalized = normalize_path(&target_path);

    // Convert the normalized path to a URL-like path
    // The path should be relative to the site root
    let url_path = normalized
        .to_string_lossy()
        .replace("\\", "/"); // Handle Windows paths

    // Reconstruct with anchor if present
    let result = if anchor.is_empty() {
        format!("/{}", url_path)
    } else {
        format!("{}#{}", url_path, anchor)
    };

    Some(result)
}

/// Normalize a path by resolving `.` and `..` components
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = PathBuf::new();

    while let Some(component) = components.next() {
        match component {
            std::path::Component::ParentDir => {
                ret.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(c) => {
                ret.push(c);
            }
            other => {
                ret.push(other);
            }
        }
    }

    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{PageSource, SiteIndex, SourceFormat};
    use crate::renderer::RenderContext;

    #[test]
    fn resolve_relative_image_path() -> miette::Result<()> {
        let mut site = SiteIndex::default();
        site.add_page(PageSource::from_string(
            "_posts/2012-10-14-hello.md",
            SourceFormat::Markdown,
            "",
        ));
        site.add_page(PageSource::from_string(
            "_posts/2013-10-14-page2.md",
            SourceFormat::Markdown,
            "![an image](../images/my_image.png)",
        ));

        let code_formatter = crate::renderer::CodeFormatter::new();
        let render_page = site
            .find_page_by_source_path(&PathBuf::from("_posts/2013-10-14-page2.md"))
            .unwrap();

        let rendered_page =
            RenderContext::run_dcx(&site, &code_formatter, |rcx| render_page.render(&rcx))?;

        assert!(rendered_page
            .rendered_contents()
            .contains("<img src=\"/images/my_image.png\""));

        Ok(())
    }

    #[test]
    fn resolve_relative_image_path_with_anchor() -> miette::Result<()> {
        let mut site = SiteIndex::default();
        site.add_page(PageSource::from_string(
            "_posts/2012-10-14-hello.md",
            SourceFormat::Markdown,
            "",
        ));
        site.add_page(PageSource::from_string(
            "_posts/2013-10-14-page2.md",
            SourceFormat::Markdown,
            "![an image](../images/my_image.png#section)",
        ));

        let code_formatter = crate::renderer::CodeFormatter::new();
        let render_page = site
            .find_page_by_source_path(&PathBuf::from("_posts/2013-10-14-page2.md"))
            .unwrap();

        let rendered_page =
            RenderContext::run_dcx(&site, &code_formatter, |rcx| render_page.render(&rcx))?;

        assert!(rendered_page
            .rendered_contents()
            .contains("<img src=\"/images/my_image.png#section\""));

        Ok(())
    }

    #[test]
    fn preserve_absolute_image_urls() -> miette::Result<()> {
        let mut site = SiteIndex::default();
        site.add_page(PageSource::from_string(
            "_posts/2013-10-14-page2.md",
            SourceFormat::Markdown,
            "![an image](https://example.com/image.png)",
        ));

        let code_formatter = crate::renderer::CodeFormatter::new();
        let render_page = site
            .find_page_by_source_path(&PathBuf::from("_posts/2013-10-14-page2.md"))
            .unwrap();

        let rendered_page =
            RenderContext::run_dcx(&site, &code_formatter, |rcx| render_page.render(&rcx))?;

        assert!(rendered_page
            .rendered_contents()
            .contains("<img src=\"https://example.com/image.png\""));

        Ok(())
    }
}

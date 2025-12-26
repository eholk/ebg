//! Rendering sites into sitemap.xml files
//!
//! This module generates XML sitemaps according to the sitemaps.org protocol.
//! The generated sitemap includes:
//! - Main site URL with high priority and daily change frequency
//! - All blog posts with last modification date and medium priority
//! - All regular pages with lower priority
//! - Category pages with medium priority
//!
//! See: https://www.sitemaps.org/protocol.html

use std::io::Write;

use miette::Diagnostic;
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesText, Event::*},
};
use thiserror::Error;

use crate::{
    index::{PageMetadata, SiteMetadata},
    renderer::RenderedSite,
};

#[derive(Error, Debug, Diagnostic)]
pub enum SitemapError {
    #[error("xml generation")]
    XmlError(
        #[source]
        #[from]
        quick_xml::Error,
    ),
    #[error("writing xml")]
    IoError(
        #[source]
        #[from]
        std::io::Error,
    ),
}

pub(crate) fn generate_sitemap(
    site: &RenderedSite,
    out: impl Write,
) -> std::result::Result<(), SitemapError> {
    let mut writer = Writer::new(out);

    writer.write_event(Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    writer
        .create_element("urlset")
        .with_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"))
        .write_inner_content(|writer: &mut Writer<_>| -> Result<(), _> {
            // Add main site URL
            writer.create_element("url").write_inner_content(
                |writer: &mut Writer<_>| -> Result<(), _> {
                    writer
                        .create_element("loc")
                        .write_text_content(BytesText::new(site.base_url()))?;
                    writer
                        .create_element("changefreq")
                        .write_text_content(BytesText::new("daily"))?;
                    writer
                        .create_element("priority")
                        .write_text_content(BytesText::new("1.0"))?;
                    Ok(())
                },
            )?;

            // Add all pages (posts and regular pages)
            for page in site.all_pages() {
                let page_url = format!("{}/{}", site.base_url(), page.url());
                writer.create_element("url").write_inner_content(
                    |writer: &mut Writer<_>| -> Result<(), _> {
                        writer
                            .create_element("loc")
                            .write_text_content(BytesText::new(&page_url))?;

                        // Add last modification date if available (for posts)
                        if let Some(publish_date) = page.publish_date() {
                            writer
                                .create_element("lastmod")
                                .write_text_content(BytesText::new(
                                    &publish_date.format("%Y-%m-%d").to_string(),
                                ))?;
                        }

                        // Set change frequency based on whether it's a post or regular page
                        let changefreq = if page.source.is_post() {
                            "monthly"
                        } else {
                            "yearly"
                        };

                        writer
                            .create_element("changefreq")
                            .write_text_content(BytesText::new(changefreq))?;

                        // Set priority - posts get higher priority than regular pages
                        let priority = if page.source.is_post() { "0.8" } else { "0.6" };

                        writer
                            .create_element("priority")
                            .write_text_content(BytesText::new(priority))?;

                        Ok(())
                    },
                )?;
            }

            // Add category pages if they exist
            for (category, _) in site.categories_and_pages() {
                let category_url = category.full_url(site.base_url());

                writer.create_element("url").write_inner_content(
                    |writer: &mut Writer<_>| -> Result<(), _> {
                        writer
                            .create_element("loc")
                            .write_text_content(BytesText::new(&category_url))?;
                        writer
                            .create_element("changefreq")
                            .write_text_content(BytesText::new("weekly"))?;
                        writer
                            .create_element("priority")
                            .write_text_content(BytesText::new("0.7"))?;
                        Ok(())
                    },
                )?;
            }

            Ok(())
        })?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::index::{PageSource, SiteIndex, SourceFormat};

    #[test]
    fn test_generate_sitemap_structure() -> std::result::Result<(), SitemapError> {
        // Create a simple site with default config (empty base URL)
        let mut site_index = SiteIndex::default();

        // Add a blog post
        site_index.add_page(PageSource::from_string(
            "_posts/2023-01-01-test-post.md",
            SourceFormat::Markdown,
            "---\ntitle: Test Post\ndate: 2023-01-01\n---\nThis is a test post.",
        ));

        // Add a regular page
        site_index.add_page(PageSource::from_string(
            "about.md",
            SourceFormat::Markdown,
            "---\ntitle: About\nlayout: page\n---\nAbout page content.",
        ));

        let rendered_site = site_index.render().expect("Failed to render site");

        // Generate sitemap
        let mut output = Vec::new();
        generate_sitemap(&rendered_site, &mut output)?;

        let sitemap_xml = String::from_utf8(output).unwrap();

        // Test for explicit expected XML structure with correct ordering
        let expected_patterns = [
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#,
            // Main site URL (should come first)
            r#"<url><loc></loc><changefreq>daily</changefreq><priority>1.0</priority></url>"#,
            // Blog post with date
            r#"<url><loc>/blog/2023/01/01/test-post/</loc><lastmod>2023-01-01</lastmod><changefreq>monthly</changefreq><priority>0.8</priority></url>"#,
            // Regular page
            r#"<url><loc>/about</loc><lastmod>1970-01-01</lastmod><changefreq>yearly</changefreq><priority>0.6</priority></url>"#,
            r#"</urlset>"#,
        ];

        // Verify each expected pattern appears in the correct order
        let mut last_position = 0;
        for pattern in expected_patterns {
            match sitemap_xml[last_position..].find(pattern) {
                Some(pos) => {
                    last_position += pos + pattern.len();
                }
                None => {
                    panic!("Expected pattern not found in sitemap: {}", pattern);
                }
            }
        }

        Ok(())
    }
}

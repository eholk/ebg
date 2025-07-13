//! Rendering sites into sitemap.xml files

use std::io::Write;

use quick_xml::{
    Writer,
    events::{BytesDecl, BytesText, Event::*},
};
use miette::Diagnostic;
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
                                .write_text_content(BytesText::new(&publish_date.format("%Y-%m-%d").to_string()))?;
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
                        let priority = if page.source.is_post() {
                            "0.8"
                        } else {
                            "0.6"
                        };
                        
                        writer
                            .create_element("priority")
                            .write_text_content(BytesText::new(priority))?;
                        
                        Ok(())
                    },
                )?;
            }

            // Add category pages if they exist
            for (category, _) in site.categories_and_pages() {
                let category_slug = slug::slugify(&category.name);
                let category_url = format!("{}/blog/category/{}/", site.base_url(), category_slug);
                
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
    use crate::{
        index::{PageSource, SiteIndex, SourceFormat},
    };

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
        
        // Verify the sitemap contains expected XML structure
        assert!(sitemap_xml.contains("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(sitemap_xml.contains("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
        assert!(sitemap_xml.contains("</urlset>"));
        
        // Verify main elements are present
        assert!(sitemap_xml.contains("<url>"));
        assert!(sitemap_xml.contains("</url>"));
        assert!(sitemap_xml.contains("<loc>"));
        assert!(sitemap_xml.contains("</loc>"));
        
        // Verify changefreq and priority are included
        assert!(sitemap_xml.contains("<changefreq>"));
        assert!(sitemap_xml.contains("<priority>"));
        
        // Basic sanity check - should contain references to our test pages
        assert!(sitemap_xml.contains("/blog/2023/01/01/test-post/"));
        assert!(sitemap_xml.contains("/about"));
        
        Ok(())
    }
}
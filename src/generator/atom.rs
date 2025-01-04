//! Rendering sites into atom.xml files

use std::io::Write;

use chrono::Utc;
use quick_xml::{
    events::{BytesCData, BytesDecl, BytesText, Event::*},
    Writer,
};
use thiserror::Error;

use crate::{
    index::{PageMetadata, SiteMetadata},
    renderer::RenderedSite,
};

#[derive(Error, Debug)]
pub enum AtomError {
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

pub(crate) fn generate_atom(
    site: &RenderedSite,
    out: impl Write,
) -> std::result::Result<(), AtomError> {
    let mut writer = Writer::new(out);

    writer.write_event(Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    writer
        .create_element("feed")
        .with_attribute(("xmlns", "http://www.w3.org/2005/Atom"))
        .write_inner_content(|writer: &mut Writer<_>| -> Result<(), _> {
            let atom_url = format!("{}/atom.xml", site.base_url());

            writer
                .create_element("link")
                .with_attributes([
                    ("href", atom_url.as_str()),
                    ("rel", "self"),
                    ("type", "application/atom+xml"),
                ])
                .write_empty()?;

            writer
                .create_element("link")
                .with_attributes([
                    ("href", site.base_url()),
                    ("rel", "alternate"),
                    ("type", "text/html"),
                ])
                .write_empty()?;

            writer
                .create_element("updated")
                .write_text_content(BytesText::new(&Utc::now().to_rfc3339()))?;

            writer
                .create_element("id")
                .write_text_content(BytesText::new(&atom_url))?;

            writer
                .create_element("title")
                .with_attribute(("type", "html"))
                .write_text_content(BytesText::new(site.title()))?;

            if let Some(subtitle) = site.subtitle() {
                writer
                    .create_element("subtitle")
                    .write_text_content(BytesText::new(subtitle))?;
            }

            if let Some(author) = site.author() {
                writer.create_element("author").write_inner_content(
                    |writer: &mut Writer<_>| -> Result<(), _> {
                        writer
                            .create_element("name")
                            .write_text_content(BytesText::new(author))?;
                        Ok(())
                    },
                )?;
            }

            let mut posts: Vec<_> = site.posts().collect();
            posts.sort_by_key(|b| std::cmp::Reverse(b.publish_date()));

            for post in posts.into_iter().take(10) {
                let post_url = format!("{}/{}", site.base_url(), post.url());
                writer.create_element("entry").write_inner_content(
                    |writer: &mut Writer<_>| -> Result<(), _> {
                        writer
                            .create_element("title")
                            .with_attribute(("type", "html"))
                            .write_text_content(BytesText::new(post.title()))?;
                        writer
                            .create_element("link")
                            .with_attributes([
                                ("href", post_url.as_str()),
                                ("rel", "alternate"),
                                ("type", "text/html"),
                                ("title", site.title()),
                            ])
                            .write_empty()?;
                        if let Some(published) = post.publish_date() {
                            writer.create_element("published").write_text_content(
                                BytesText::new(published.to_rfc3339().as_str()),
                            )?;
                            // TODO: find a more accurate way to do last updated.
                            writer
                                .create_element("updated")
                                .write_text_content(BytesText::new(
                                    published.to_rfc3339().as_str(),
                                ))?;
                        }
                        writer
                            .create_element("id")
                            .write_text_content(BytesText::new(post_url.as_str()))?;

                        writer
                            .create_element("content")
                            .with_attributes([("type", "html"), ("xml:base", post_url.as_str())])
                            .write_cdata_content(BytesCData::new(post.rendered_contents()))?;

                        if let Some(author) = site.author() {
                            writer.create_element("author").write_inner_content(
                                |writer: &mut Writer<_>| -> Result<(), _> {
                                    writer
                                        .create_element("name")
                                        .write_text_content(BytesText::new(author))?;
                                    Ok(())
                                },
                            )?;
                        }

                        // FIXME: Add categories for posts that have them

                        if let Some(excerpt) = post.rendered_excerpt() {
                            writer
                                .create_element("summary")
                                .with_attribute(("type", "html"))
                                .write_cdata_content(BytesCData::new(excerpt))?;
                        }

                        Ok(())
                    },
                )?;
            }

            Ok(())
        })?;

    Ok(())
}

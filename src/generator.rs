use std::path::Path;

use eyre::{ContextCompat, WrapErr};
use pathdiff::diff_paths;
use serde_json::{json, Map, Value};
use tera::Context;
use tokio::fs;
use tracing::debug;

use crate::{command_line::Options, page::Page, site::Site};

use self::atom::generate_atom;

mod atom;

pub async fn generate_site(site: &Site, options: &Options) -> eyre::Result<()> {
    for post in site.all_pages() {
        generate_page(post, site, options)
            .await
            .context("generating post")?;
    }

    for file in site.raw_files() {
        debug!(
            "copying from {}, root {}",
            file.display(),
            site.root_dir().display()
        );
        let dest = options.destination.join(
            diff_paths(file, site.root_dir()).context("computing destination path for raw file")?,
        );

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("creating target directory")?;
        }

        fs::copy(file, dest).await.context("copying raw file")?;
    }

    generate_atom(
        site,
        std::fs::File::create(options.destination.join("atom.xml")).context("creating atom.xml")?,
    )
    .context("generating atom feed")?;

    Ok(())
}

/// Converts an object into a format that can be passed to a Tera template
trait ToValue {
    fn value(&self) -> Value;
}

impl ToValue for Page {
    fn value(&self) -> Value {
        let mut page = Map::new();
        page.insert("title".to_string(), json!(self.title()));
        page.insert("url".to_string(), json!(Path::new("/").join(self.url())));
        if let Some(date) = self.publish_date() {
            page.insert("date".to_string(), json!(date));
        }
        page.insert(
            "excerpt".to_string(),
            json!(self.rendered_excerpt().unwrap_or_default()),
        );
        page.insert("content".to_string(), json!(self.rendered_contents()));
        page.into()
    }
}

impl ToValue for Site {
    fn value(&self) -> Value {
        let mut site = [("url".to_string(), json!(self.base_url()))]
            .into_iter()
            .collect::<Map<_, _>>();

        let mut posts = self.posts().collect::<Vec<_>>();
        posts.sort_by_key(|b| std::cmp::Reverse(b.publish_date()));

        site.insert(
            "posts".to_string(),
            json!(posts.into_iter().map(ToValue::value).collect::<Vec<_>>()),
        );
        site.into()
    }
}

async fn generate_page(page: &Page, site: &Site, options: &Options) -> eyre::Result<()> {
    debug!(
        "post frontmatter:\n{}\n\nparsed as: {:#?}",
        page.raw_frontmatter().unwrap_or("None"),
        page.frontmatter(),
    );

    let dest = options.destination.join(page.url()).join("index.html");

    debug!("destination path: {}", dest.display());

    let content = page.rendered_contents();

    debug!("post template: {:?}", page.template());
    let content = match page.template() {
        Some(template) => {
            let mut context = Context::new();
            context.insert("site", &site.value());
            context.insert("page", &page.value());

            let content_template = site
                .config()
                .macros
                .iter()
                .map(|(name, path)| format!("{{% import \"{}\" as {name} %}}", path.display()))
                .collect::<Vec<_>>()
                .join("")
                + content;
            let mut templates = site.templates().clone();
            let content = templates.render_str(&content_template, &context)?;

            context.insert("content", &content);
            site.templates()
                .render(&format!("{template}.html"), &context)
                .context("rendering template")?
        }
        None => content.to_string(),
    };

    tokio::fs::create_dir_all(dest.parent().unwrap())
        .await
        .context("creating destination directory")?;

    tokio::fs::write(dest, content)
        .await
        .context("writing output")?;

    Ok(())
}

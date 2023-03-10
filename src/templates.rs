//! Code for loading templates, plus any custom filters we use.

use std::path::Path;

use eyre::{ContextCompat, WrapErr};
use tera::Tera;
use tracing::debug;

pub fn create_template_engine(root_dir: &Path) -> eyre::Result<Tera> {
    let template_path = std::env::current_dir()?
        .join(root_dir)
        .join("templates")
        .join("**")
        .join("*.html");
    debug!("loading templates from {}", template_path.display());
    let mut tera = Tera::new(template_path.to_str().context("invalid template path")?)
        .context("loading templates")?;
    // Disable escaping since we are a static site and so we consider all our input trusted.
    tera.autoescape_on(vec![]);

    debug!(
        "found templates:\n{}",
        tera.get_template_names().collect::<Vec<_>>().join("\n")
    );

    Ok(tera)
}

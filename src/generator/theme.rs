//! Code for loading templates, plus any custom filters we use.

use std::path::Path;

use tera::Tera;
use tracing::debug;

use crate::index::Config;

use super::GeneratorError;

pub fn create_template_engine(root_dir: &Path, config: &Config) -> Result<Tera, GeneratorError> {
    let template_path = std::env::current_dir()
        .unwrap()
        .join(root_dir)
        .join(
            config
                .theme
                .as_ref()
                .map_or(Path::new("theme"), |p| p.as_path()),
        )
        .join("**")
        .join("*.html");
    debug!("loading templates from {}", template_path.display());
    // FIXME: report error to caller instead of using expect
    let mut tera = Tera::new(template_path.to_str().expect("invalid template path"))
        .map_err(|e| GeneratorError::LoadTemplates(Box::new(e)))?;
    // Disable escaping since we are a static site and so we consider all our input trusted.
    tera.autoescape_on(vec![]);

    debug!(
        "found templates:\n{}",
        tera.get_template_names().collect::<Vec<_>>().join("\n")
    );

    Ok(tera)
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eyre::WrapErr;
use futures::StreamExt;
use serde::Deserialize;
use tera::Tera;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

use crate::{
    markdown::CodeFormatter,
    page::{Page, PageKind},
    theme::create_template_engine,
};

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub subtitle: Option<String>,
    pub posts: Option<PathBuf>,
    pub theme: Option<PathBuf>,
    #[serde(default)]
    pub content: Vec<PathBuf>,
    #[serde(default)]
    pub macros: HashMap<String, PathBuf>,
}

#[derive(Default)]
pub struct Site {
    config: Config,
    root_dir: PathBuf,
    pages: Vec<Page>,
    raw_files: Vec<PathBuf>,
    templates: Tera,
}

impl Site {
    pub async fn from_directory(
        path: impl Into<PathBuf>,
        include_unpublished: bool,
    ) -> eyre::Result<Self> {
        let root_dir = path.into();

        let config: Config = toml::from_str(
            &fs::read_to_string(root_dir.join("Site.toml"))
                .await
                .context("reading Site.toml")?,
        )
        .context("parsing Site.toml")?;

        let mut pages = vec![];
        let mut raw_files = Vec::new();

        pages.extend(
            load_posts(
                &root_dir.join(config.posts.as_ref().unwrap_or(&"_posts".into())),
                &root_dir,
                include_unpublished,
            )
            .await
            .context("loading posts")?,
        );

        for path in config.content.iter() {
            #[allow(clippy::match_single_binding)]
            match load_directory(root_dir.join(path), &root_dir, include_unpublished)
                .await
                .with_context(|| format!("loading {}", path.display()))?
            {
                (new_pages, files) => {
                    pages.extend(new_pages.into_iter());
                    raw_files.extend(files.into_iter());
                }
            }
        }

        let templates = create_template_engine(&root_dir, &config).context("loading templates")?;
        let code_formatter = CodeFormatter::new();

        for page in pages.iter_mut() {
            page.render(&code_formatter);
        }

        Ok(Site {
            config,
            root_dir,
            pages,
            raw_files,
            templates,
        })
    }

    pub fn posts(&self) -> impl Iterator<Item = &Page> {
        self.pages
            .iter()
            .filter(|post| post.kind() == PageKind::Post)
    }

    pub fn all_pages(&self) -> impl Iterator<Item = &Page> {
        self.pages.iter()
    }

    pub fn templates(&self) -> &Tera {
        &self.templates
    }

    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    pub fn raw_files(&self) -> impl Iterator<Item = &Path> {
        self.raw_files.iter().map(AsRef::as_ref)
    }

    pub fn base_url(&self) -> &str {
        match &self.config.url {
            Some(url) => url,
            None => "",
        }
    }

    pub fn title(&self) -> &str {
        &self.config.title
    }

    pub fn subtitle(&self) -> Option<&str> {
        self.config.subtitle.as_deref()
    }

    pub fn author(&self) -> Option<&str> {
        self.config.author.as_deref()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}

async fn load_posts(
    path: &Path,
    root_dir: &Path,
    include_unpublished: bool,
) -> eyre::Result<Vec<Page>> {
    if !path.is_dir() {
        return Ok(vec![]);
    }

    let mut posts = vec![];
    let mut dirstream = ReadDirStream::new(
        fs::read_dir(path)
            .await
            .context("could not read directory")?,
    );
    while let Some(entry) = dirstream.next().await {
        let entry = entry.context("reading directory entry")?;
        let page = Page::from_file(entry.path(), root_dir)
            .await
            .context("parsing post")?;

        if page.published() || include_unpublished {
            posts.push(page)
        }
    }

    Ok(posts)
}

/// Loads the files in a directory, returning those that need further processing as pages
/// and those that can be copied verbatim to the destination directory
async fn load_directory(
    path: impl AsRef<Path>,
    root_dir: &Path,
    include_unpublished: bool,
) -> eyre::Result<(Vec<Page>, Vec<PathBuf>)> {
    let path = path.as_ref();
    let mut pages = vec![];
    let mut raw_files = vec![];

    if path.is_file() {
        if let Ok(page) = Page::from_file(path, root_dir).await {
            if page.published() || include_unpublished {
                return Ok((vec![page], vec![]));
            } else {
                return Ok((vec![], vec![]));
            }
        } else {
            return Ok((vec![], vec![path.into()]));
        }
    }

    let mut walk = async_walkdir::WalkDir::new(path);
    while let Some(result) = walk.next().await {
        let entry = result.context("reading directory entry")?;

        if !entry.file_type().await?.is_file() {
            continue;
        }

        let filename = entry.path();
        if let Ok(page) = Page::from_file(&filename, root_dir).await {
            if page.published() || include_unpublished {
                pages.push(page)
            }
        } else {
            raw_files.push(filename)
        }
    }

    Ok((pages, raw_files))
}

#[cfg(test)]
mod test {
    use super::Config;

    #[test]
    fn parse_site_config() {
        let config = r#"url = "https://example.com"
        title = "example site"
        "#;

        let config: Config = toml::from_str(config).unwrap();

        assert_eq!(config.url, Some("https://example.com".to_string()));
    }
}

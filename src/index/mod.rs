//! Contains data structures that represent the full site.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use futures::StreamExt;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

mod page;

pub use page::{PageKind, PageMetadata, PageSource, SourceFormat};

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

#[derive(Error, Debug)]
pub enum IndexError {

}

/// Holds what is essentially metadata about a site
///
/// This allows us to refer to the site as a whole during page rendering, which
/// in turn enables things like resolving relative links.
#[derive(Default)]
pub struct SiteIndex {
    config: Config,
    root_dir: PathBuf,
    pages: Vec<PageSource>,
    raw_files: Vec<PathBuf>,
}

impl SiteIndex {
    pub async fn from_directory(
        path: impl Into<PathBuf>,
        include_unpublished: bool,
    ) -> Result<Self, IndexError> {
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

        Ok(SiteIndex {
            config,
            root_dir,
            pages,
            raw_files,
        })
    }

    pub fn posts(&self) -> impl Iterator<Item = &PageSource> {
        self.pages
            .iter()
            .filter(|post| post.kind() == PageKind::Post)
    }

    pub fn all_pages(&self) -> impl Iterator<Item = &PageSource> {
        self.pages.iter()
    }

    /// Finds a page given its source path
    ///
    /// The path should be given relative to the site root.
    pub fn find_page_by_source_path(&self, path: &Path) -> Option<&PageSource> {
        self.pages.iter().find(|page| page.source_path() == path)
    }

    /// Adds a new page to the site
    /// 
    /// This generally shouldn't be needed since pages are loaded from the filesystem,
    /// but it can be helpful in building mock sites for testing.
    pub fn add_page(&mut self, page: PageSource) {
        self.pages.push(page);
    }
}

/// Accessor methods for various kinds of site metadata
pub trait SiteMetadata {
    fn config(&self) -> &Config;
    fn base_url(&self) -> &str; // FIXME: use a URL type
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;
    fn author(&self) -> Option<&str>;
    fn root_dir(&self) -> &PathBuf;
    fn num_pages(&self) -> usize;
    fn raw_files(&self) -> impl Iterator<Item = &Path>
    where
        Self: Sized;
}

impl SiteMetadata for SiteIndex {
    fn base_url(&self) -> &str {
        match &self.config.url {
            Some(url) => url,
            None => "",
        }
    }

    fn title(&self) -> &str {
        &self.config.title
    }

    fn subtitle(&self) -> Option<&str> {
        self.config.subtitle.as_deref()
    }

    fn author(&self) -> Option<&str> {
        self.config.author.as_deref()
    }

    fn config(&self) -> &Config {
        &self.config
    }

    fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    fn num_pages(&self) -> usize {
        self.pages.len()
    }

    fn raw_files(&self) -> impl Iterator<Item = &Path> {
        self.raw_files.iter().map(AsRef::as_ref)
    }
}

async fn load_posts(
    path: &Path,
    root_dir: &Path,
    include_unpublished: bool,
) -> Result<Vec<PageSource>, IndexError> {
    if !path.is_dir() {
        return Ok(vec![]);
    }

    let mut posts = vec![];
    let mut dir_stream = ReadDirStream::new(
        fs::read_dir(path)
            .await
            .context("could not read directory")?,
    );
    while let Some(entry) = dir_stream.next().await {
        let entry = entry.context("reading directory entry")?;
        let page = PageSource::from_file(entry.path(), root_dir)
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
) -> Result<(Vec<PageSource>, Vec<PathBuf>), IndexError> {
    let path = path.as_ref();
    let mut pages = vec![];
    let mut raw_files = vec![];

    if path.is_file() {
        if let Ok(page) = PageSource::from_file(path, root_dir).await {
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
        if let Ok(page) = PageSource::from_file(&filename, root_dir).await {
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

//! Contains data structures that represent the full site.

use std::{
    collections::{BTreeMap, HashMap},
    ops::Index,
    path::{Path, PathBuf},
};

use futures::StreamExt;
use miette::{Diagnostic, Severity};
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

mod page;

pub use page::{PageKind, PageMetadata, PageSource, SourceFormat};

use self::page::PageLoadError;

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub subtitle: Option<String>,
    pub posts: Option<PathBuf>,
    pub theme: Option<PathBuf>,
    #[serde(default)]
    pub content: Vec<PathBuf>,
    #[serde(default)]
    pub macros: HashMap<String, PathBuf>,
    /// Options that are passed directly to to the theme
    ///
    /// Within theme templates, these are available under the `theme` variable.
    #[serde(default)]
    pub theme_opts: serde_json::Value,
}

#[non_exhaustive]
#[derive(Diagnostic, Error, Debug)]
pub enum IndexError {
    #[error("reading directory entry")]
    ReadingDirectoryEntry(#[source] std::io::Error),
    #[error("reading directory entry")]
    WalkdirReadingDirectoryEntry(#[source] walkdir::Error),
    #[error("invalid post filename: `{}`", .0.display())]
    InvalidFilename(PathBuf),
    #[error("reading post contents")]
    ReadingPostContents(#[source] std::io::Error),
    #[error("reading Site.toml")]
    ReadingConfigFile(#[source] std::io::Error),
    #[error("parsing Site.toml")]
    ParsingConfigFile(#[source] Box<dyn std::error::Error + Send + Sync>),
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
    categories: BTreeMap<String, Category>,
}

impl SiteIndex {
    pub async fn from_directory(
        path: impl Into<PathBuf>,
        include_unpublished: bool,
    ) -> Result<Self, IndexError> {
        let root_dir = path.into();

        // FIXME: give friendly error reports for bad config files
        let config: Config = toml::from_str(
            &std::fs::read_to_string(root_dir.join("Site.toml"))
                .map_err(IndexError::ReadingConfigFile)?,
        )
        .map_err(|e| IndexError::ParsingConfigFile(Box::new(e)))?;

        let mut pages = vec![];
        let mut raw_files = Vec::new();

        pages.extend(
            load_posts(
                &root_dir.join(config.posts.as_ref().unwrap_or(&"_posts".into())),
                &root_dir,
                include_unpublished,
            )
            .await?,
        );

        for path in config.content.iter() {
            match load_directory(root_dir.join(path), &root_dir, include_unpublished).await? {
                (new_pages, files) => {
                    pages.extend(new_pages.into_iter());
                    raw_files.extend(files.into_iter());
                }
            }
        }

        // Gather all the category information
        let mut categories = BTreeMap::new();
        for (id, page) in pages.iter().enumerate() {
            match page.categories() {
                Some(page_categories) => {
                    for category in page_categories {
                        categories
                            .entry(category.to_string())
                            .or_insert_with(|| Category {
                                name: category.to_string(),
                                posts: vec![],
                            })
                            .posts
                            .push(PageId(id));
                    }
                }
                None => (),
            }
        }

        Ok(SiteIndex {
            config,
            root_dir,
            pages,
            raw_files,
            categories,
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

    pub fn categories(&self) -> impl Iterator<Item = &Category> {
        self.categories.values()
    }

    pub fn find_pages_in_category(&self, category: &str) -> impl Iterator<Item = &PageSource> {
        self.categories
            .get(category)
            .into_iter()
            .flat_map(|cat| cat.posts.iter())
            .map(|id| &self.pages[id.0])
    }
}

impl Index<PageId> for SiteIndex {
    type Output = PageSource;

    fn index(&self, id: PageId) -> &Self::Output {
        &self.pages[id.0]
    }
}

/// Accessor methods for various kinds of site metadata
pub trait SiteMetadata {
    fn config(&self) -> &Config;
    fn base_url(&self) -> &str; // FIXME: use a URL type
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;
    fn author(&self) -> Option<&str>;
    fn author_email(&self) -> Option<&str>;
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

    fn author_email(&self) -> Option<&str> {
        self.config.author_email.as_deref()
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

#[derive(Debug, Diagnostic, Error)]
#[diagnostic(severity(warning))]
#[error("skipping post with filename `{filename}`")]
struct SkippedPost {
    #[source_code]
    filename: String,
    #[diagnostic_source]
    reason: PageLoadError,
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
            .map_err(IndexError::ReadingDirectoryEntry)?,
    );
    while let Some(entry) = dir_stream.next().await {
        let entry = entry.map_err(IndexError::ReadingDirectoryEntry)?;
        let page = match PageSource::from_file(entry.path(), root_dir).await {
            Ok(page) => page,
            Err(e) if e.severity() <= Some(Severity::Warning) => {
                println!(
                    "{:?}",
                    miette::Report::new(SkippedPost {
                        filename: entry.path().display().to_string(),
                        reason: e,
                    })
                );
                continue;
            }
            Err(e) => panic!("I don't know what to do with this error: {}", e),
        };

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

    let walk = walkdir::WalkDir::new(path);
    for result in walk {
        let entry = result.map_err(IndexError::WalkdirReadingDirectoryEntry)?;

        if !entry.file_type().is_file() {
            continue;
        }

        let filename = entry.path();
        if let Ok(page) = PageSource::from_file(&filename, root_dir).await {
            if page.published() || include_unpublished {
                pages.push(page)
            }
        } else {
            raw_files.push(filename.to_path_buf())
        }
    }

    Ok((pages, raw_files))
}

pub struct PageId(usize);

pub struct Category {
    pub(crate) name: String,
    posts: Vec<PageId>,
}

impl Category {
    /// Generate the slug for this category
    pub fn slug(&self) -> String {
        slug::slugify(&self.name)
    }

    /// Generate a relative URL path for this category (e.g., "/blog/category/tech/")
    pub fn url_path(&self) -> String {
        format!("/blog/category/{}/", self.slug())
    }

    /// Generate a full URL for this category with the given base URL (e.g., "https://example.com/blog/category/tech/")
    pub fn full_url(&self, base_url: &str) -> String {
        format!("{}/blog/category/{}/", base_url, self.slug())
    }
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

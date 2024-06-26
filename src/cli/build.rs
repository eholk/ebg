use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use ebg::{
    generator::{self, GeneratorContext, Observer},
    index::{PageMetadata, SiteIndex, SiteMetadata},
};
use indicatif::{MultiProgress, ProgressBar};
use miette::{Context, IntoDiagnostic};
use tokio::runtime::Runtime;
use tracing::info;

enum ProgressState {
    NotStarted,
    LoadingSite(ProgressBar),
    BuildingSite {
        header: ProgressBar,
        pages: ProgressBar,
    },
    Complete,
}

struct BuildStatusViewer {
    progress: MultiProgress,
    state: Mutex<ProgressState>,
}

impl BuildStatusViewer {
    fn new() -> Self {
        Self {
            progress: MultiProgress::new(),
            state: Mutex::new(ProgressState::NotStarted),
        }
    }
}

impl Observer for BuildStatusViewer {
    fn begin_load_site(&self) {
        let progress = self.progress.add(ProgressBar::new_spinner());
        progress.set_message("Loading site directory");
        progress.enable_steady_tick(Duration::from_millis(250));
        *self.state.lock().unwrap() = ProgressState::LoadingSite(progress);
    }

    fn end_load_site(&self, site: &dyn SiteMetadata) {
        let mut state = self.state.lock().unwrap();

        // cleanup the old state
        if let ProgressState::LoadingSite(progress) = &*state {
            progress.finish();
            // self.progress.remove(progress);
        }

        // set up the new state
        let header = self.progress.add(ProgressBar::new_spinner());
        header.set_message("Building pages");
        let pages = self.progress.add(ProgressBar::new(site.num_pages() as u64));
        *state = ProgressState::BuildingSite { header, pages };
    }

    fn end_page(&self, _page: &dyn PageMetadata) {
        let state = self.state.lock().unwrap();
        if let ProgressState::BuildingSite { header, pages } = &*state {
            pages.inc(1);
            header.tick();
        }
    }

    fn site_complete(&self, _site: &dyn SiteMetadata) {
        let mut state = self.state.lock().unwrap();
        if let ProgressState::BuildingSite { header, pages } = &*state {
            pages.finish();
            header.finish();
            self.progress.remove(pages);
            self.progress.remove(header);
            *state = ProgressState::Complete;
        }
    }
}

impl super::Command for generator::Options {
    fn run(self) -> miette::Result<()> {
        let path = find_site_root(self.path.as_deref()).context("finding Site.toml")?;
        info!("building blog from {}", path.display());

        let start_time = Instant::now();
        let progress = BuildStatusViewer::new();

        Runtime::new().into_diagnostic()?.block_on(async move {
            progress.begin_load_site();
            let site = SiteIndex::from_directory(&path, self.unpublished).await?;
            progress.end_load_site(&site);

            let site = site.render()?;

            let gcx = GeneratorContext::new(&site, &self)?;

            gcx.generate_site(&site).await?;
            progress.site_complete(&site);

            let elapsed = start_time.elapsed();

            println!("Built site in {:.2?}", elapsed);

            Ok(())
        })
    }
}

pub(crate) fn find_site_root(path: Option<&Path>) -> miette::Result<PathBuf> {
    let mut path = path.unwrap_or(Path::new(".")).to_path_buf();
    loop {
        if path.join("Site.toml").exists() {
            return Ok(path.clone());
        }

        path = match path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => miette::bail!("could not find Site.toml in any parent directory"),
        };
    }
}

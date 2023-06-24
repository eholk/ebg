use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use ebg::{
    generator::{self, generate_site, Observer, Options},
    page::Page,
    site::Site,
};
use eyre::Context;
use indicatif::{HumanDuration, MultiProgress, ProgressBar};
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

    fn end_load_site(&self, site: &Site) {
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

    fn end_page(&self, _page: &Page) {
        let state = self.state.lock().unwrap();
        if let ProgressState::BuildingSite { header, pages } = &*state {
            pages.inc(1);
            header.tick();
        }
    }

    fn site_complete(&self, _site: &Site) {
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
    async fn run(self) -> eyre::Result<()> {
        let path = find_site_root(&self).context("finding Site.toml")?;
        info!("building blog from {}", path.display());

        let start_time = Instant::now();
        let progress = BuildStatusViewer::new();

        progress.begin_load_site();
        let site = Site::from_directory(&path, self.unpublished)
            .await
            .context("loading site content")?;
        progress.end_load_site(&site);

        generate_site(&site, &self, Some(&progress))
            .await
            .context("generating site")?;
        progress.site_complete(&site);

        let elapsed = start_time.elapsed();

        println!("Built site in {}", HumanDuration(elapsed));

        Ok(())
    }
}

pub(crate) fn find_site_root(options: &Options) -> eyre::Result<PathBuf> {
    let mut path = options.path.clone().unwrap_or(".".into());
    loop {
        if path.join("Site.toml").exists() {
            return Ok(path.clone());
        }

        path = match path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => eyre::bail!("could not find ebg.toml in any parent directory"),
        };
    }
}

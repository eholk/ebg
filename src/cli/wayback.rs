use clap::Parser;
use ebg::wayback::Wayback;
use miette::IntoDiagnostic;
use url::Url;

use super::Command;

#[derive(Parser)]
pub struct WaybackOptions {
    /// The URL to archive.
    url: String,
}

impl Command for WaybackOptions {
    fn run(self) -> miette::Result<()> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let wayback = Wayback::with_credentials(
                    std::env::var("WAYBACK_ACCESS_KEY").unwrap(),
                    std::env::var("WAYBACK_SECRET_KEY").unwrap(),
                );

                let job = wayback
                    .begin_save_page(&Url::parse(&self.url).into_diagnostic()?)
                    .await?;

                println!("Job ID: {}", job.job_id());

                let mut status = wayback.job_status(&job).await?;

                while status.status() != "success" {
                    println!("Status: {}", status.status());

                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    status = wayback.job_status(&job).await?;
                }

                println!("Job result: {:#?}", status);

                Ok(())
            })
    }
}

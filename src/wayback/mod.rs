//! API for working with the Save Page Now API of the Wayback Machine.
//!
//! This could probably be factored into its own crate at some point, but for
//! now it's internal to ebg.
//!
//! This uses the Save Page Now (SPN2) API, which is documented at:
//!
//! <https://docs.google.com/document/d/1Nsv52MvSjbLb2PCpHlat0gkzw0EvtSgpKHu4mk0MnrA/edit>
//!
//! For now this does not aim to be a complete implementation of the API, just
//! enough to support the features EBG needs.

use std::{collections::HashMap, time::Duration};

use serde::Deserialize;
use thiserror::Error;
use url::Url;

/// The top level client for the Wayback Machine's Save Page Now API.
pub struct Wayback {
    access_key: String,
    secret_key: String,
    client: reqwest::Client,
}

impl Wayback {
    /// Creates a new [`Wayback`] client using the given credentials.
    ///
    /// Credentials can be obtained from <https://archive.org/account/s3.php>.
    pub fn with_credentials(access_key: impl ToString, secret_key: impl ToString) -> Self {
        Self {
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Begins a new job to save the given page.
    ///
    /// On success, returns a [`Job`] object that can be used to check the
    /// status.
    pub async fn begin_save_page(&self, url: &Url) -> Result<Job, Error> {
        let response = self
            .client
            .post("https://web.archive.org/save")
            .header(
                "Authorization",
                format!("LOW {}:{}", self.access_key, self.secret_key),
            )
            .header("Accept", "application/json")
            .form(&[("url", url.as_str())])
            .send()
            .await?;

        let job = response.json().await?;

        Ok(job)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error")]
    HttpError(
        #[from]
        #[source]
        reqwest::Error,
    ),
}

#[derive(Deserialize)]
pub struct Job {
    url: Option<Url>,
    job_id: String,
    message: Option<String>,
}

#[derive(Deserialize)]
pub struct Status {
    counters: HashMap<String, u32>,
    delay_wb_availability: bool,
    duration_sec: f64,
    http_status: u16,
    job_id: String,
    original_url: Url,
    outlinks: Vec<Url>,
    resources: Vec<Url>,
    status: String,
    timestamp: String,
}

#[cfg(test)]
mod test {
    use miette::IntoDiagnostic;

    #[test]
    fn deserialize_jobs() -> miette::Result<()> {
        // some example responses from the API:
        //
        // Successful job submission:
        // {"url":"https://theincredibleholk.org/about/","job_id":"spn2-ffa890ab71a52c6ca87389d4f214becedbaa275a"}
        //
        // One that's already been submitted:
        // {"url":"https://theincredibleholk.org/about/","job_id":"spn2-ffa890ab71a52c6ca87389d4f214becedbaa275a","message":"The same snapshot had been made 1 second ago. You can make new capture of this URL after 1 hour."}

        serde_json::from_str::<super::Job>(
            r#"{"url":"https://theincredibleholk.org/about/","job_id":"spn2-ffa890ab71a52c6ca87389d4f214becedbaa275a"}"#,
        ).into_diagnostic()?;

        serde_json::from_str::<super::Job>(
            r#"{"url":"https://theincredibleholk.org/about/","job_id":"spn2-ffa890ab71a52c6ca87389d4f214becedbaa275a","message":"The same snapshot had been made 1 second ago. You can make new capture of this URL after 1 hour."}"#,
        ).into_diagnostic()?;

        Ok(())
    }

    #[test]
    fn deserialize_job_status() -> miette::Result<()> {
        // {"counters":{"embeds":8,"outlinks":47},"delay_wb_availability":true,"duration_sec":12.72,"http_status":200,"job_id":"spn2-c093004522eaa435107c0d9ee8aac46a17199841","original_url":"https://theincredibleholk.org/","outlinks":["https://github.com/eholk","https://mastodon.social/@theincredibleholk","https://theincredibleholk.org/blog/2023/07/11/how-to-elect-rust-project-directors/","https://theincredibleholk.org/blog/2023/01/25/hello-from-erics-blog-generator/","https://rust-lang.zulipchat.com/#narrow/stream/213817-t-lang/topic/Where.20to.20talk.20about.20.60try.20.7B.7D.60.2C.20.60yeet.60.2C.20etc.3F","https://theincredibleholk.org/blog/2023/11/08/cancellation-async-state-machines/","https://theincredibleholk.org/office-hours/","https://ryanlevick.com/","https://smallcultfollowing.com/babysteps/blog/2023/02/01/async-trait-send-bounds-part-1-intro/","https://github.com/rust-lang/rust/pull/118457","https://veykril.github.io/about/","https://github.com/eholk/ebg","https://theincredibleholk.org/blog/2023/06/20/rust-leadership-council/","https://theincredibleholk.org/blog/2023/12/15/rethinking-rusts-function-declaration-syntax/","https://theincredibleholk.org/blog/2023/06/23/an-exercise-on-culture/","https://theincredibleholk.org/atom.xml","https://yaah.dev/","https://github.com/rust-lang/rust/pull/118420","https://theincredibleholk.org/about/","https://github.com/rust-lang/wg-async/issues/297","https://www.prdaily.com/how-microsoft-manages-culture-change/","https://theincredibleholk.org/blog/2023/11/14/a-mechanism-for-async-cancellation/","https://theincredibleholk.org/blog/2023/02/16/lightweight-predictable-async-send-bounds/","https://theincredibleholk.org/blog/2023/01/24/who-makes-the-boxes/","http://www.apache.org/licenses/LICENSE-2.0","https://smallcultfollowing.com/babysteps/blog/2023/02/13/return-type-notation-send-bounds-part-2/","https://github.com/orgs/rust-lang/projects/28/views/1","https://theincredibleholk.org/blog/2023/02/13/inferred-async-send-bounds/","https://doc.rust-lang.org/std/ops/trait.Try.html#impl-Try-for-Option%3CT%3E","http://creativecommons.org/licenses/by-nc/4.0/","https://theincredibleholk.org/blog/archives/","https://theincredibleholk.org/papers/"],"resources":["https://theincredibleholk.org/","https://theincredibleholk.org/assets/main.css","https://theincredibleholk.org/images/cc-by-nc-4.0-88x31.png","https://theincredibleholk.org/assets/fonts/BerkeleyMono-Bold.woff2","https://theincredibleholk.org/assets/fonts/BerkeleyMono-Regular.woff2","https://theincredibleholk.org/assets/fonts/D-DIN.otf","https://theincredibleholk.org/assets/fonts/BerkeleyMono-Italic.woff2","https://theincredibleholk.org/assets/fonts/D-DIN-Italic.otf"],"status":"success","timestamp":"20240104034229"}
        let raw_json = r#"{"counters":{"embeds":8,"outlinks":47},"delay_wb_availability":true,"duration_sec":12.72,"http_status":200,"job_id":"spn2-c093004522eaa435107c0d9ee8aac46a17199841","original_url":"https://theincredibleholk.org/","outlinks":["https://github.com/eholk","https://mastodon.social/@theincredibleholk","https://theincredibleholk.org/blog/2023/07/11/how-to-elect-rust-project-directors/","https://theincredibleholk.org/blog/2023/01/25/hello-from-erics-blog-generator/","https://rust-lang.zulipchat.com/#narrow/stream/213817-t-lang/topic/Where.20to.20talk.20about.20.60try.20.7B.7D.60.2C.20.60yeet.60.2C.20etc.3F","https://theincredibleholk.org/blog/2023/11/08/cancellation-async-state-machines/","https://theincredibleholk.org/office-hours/","https://ryanlevick.com/","https://smallcultfollowing.com/babysteps/blog/2023/02/01/async-trait-send-bounds-part-1-intro/","https://github.com/rust-lang/rust/pull/118457","https://veykril.github.io/about/","https://github.com/eholk/ebg","https://theincredibleholk.org/blog/2023/06/20/rust-leadership-council/","https://theincredibleholk.org/blog/2023/12/15/rethinking-rusts-function-declaration-syntax/","https://theincredibleholk.org/blog/2023/06/23/an-exercise-on-culture/","https://theincredibleholk.org/atom.xml","https://yaah.dev/","https://github.com/rust-lang/rust/pull/118420","https://theincredibleholk.org/about/","https://github.com/rust-lang/wg-async/issues/297","https://www.prdaily.com/how-microsoft-manages-culture-change/","https://theincredibleholk.org/blog/2023/11/14/a-mechanism-for-async-cancellation/","https://theincredibleholk.org/blog/2023/02/16/lightweight-predictable-async-send-bounds/","https://theincredibleholk.org/blog/2023/01/24/who-makes-the-boxes/","http://www.apache.org/licenses/LICENSE-2.0","https://smallcultfollowing.com/babysteps/blog/2023/02/13/return-type-notation-send-bounds-part-2/","https://github.com/orgs/rust-lang/projects/28/views/1","https://theincredibleholk.org/blog/2023/02/13/inferred-async-send-bounds/","https://doc.rust-lang.org/std/ops/trait.Try.html#impl-Try-for-Option%3CT%3E","http://creativecommons.org/licenses/by-nc/4.0/","https://theincredibleholk.org/blog/archives/","https://theincredibleholk.org/papers/"],"resources":["https://theincredibleholk.org/","https://theincredibleholk.org/assets/main.css","https://theincredibleholk.org/images/cc-by-nc-4.0-88x31.png","https://theincredibleholk.org/assets/fonts/BerkeleyMono-Bold.woff2","https://theincredibleholk.org/assets/fonts/BerkeleyMono-Regular.woff2","https://theincredibleholk.org/assets/fonts/D-DIN.otf","https://theincredibleholk.org/assets/fonts/BerkeleyMono-Italic.woff2","https://theincredibleholk.org/assets/fonts/D-DIN-Italic.otf"],"status":"success","timestamp":"20240104034229"}"#;

        serde_json::from_str::<super::Status>(raw_json).into_diagnostic()?;

        Ok(())
    }
}

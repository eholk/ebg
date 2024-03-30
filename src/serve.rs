use std::{convert::Infallible, net::SocketAddr, path::Path, time::Instant};

use clap::Args;
use ebg::{
    generator::{GeneratorContext, Options},
    index::SiteIndex,
};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use miette::IntoDiagnostic;
use notify::{Event, RecursiveMode, Watcher};
use thiserror::Error;
use tokio::runtime::Runtime;
use tracing::{debug, error, info};

use crate::cli::{build::find_site_root, Command};

#[derive(Args)]
pub struct ServerOptions {
    #[command(flatten)]
    build_opts: Options,

    #[clap(short, long, default_value_t = 4000)]
    port: u16,
}

impl Command for ServerOptions {
    fn run(self) -> miette::Result<()> {
        let rt = Runtime::new().into_diagnostic()?;
        rt.block_on(serve(self))
    }
}

#[derive(Debug)]
enum GeneratorMessage {
    Rebuild,
}

#[derive(Debug, Error)]
enum ServerError {
    #[error("could not find file to satisfy URI `{0}`")]
    PathNotFound(hyper::http::uri::Uri),
    #[error("error reading file contents")]
    ReadContents(#[source] std::io::Error),
    #[error("error building response body")]
    ResponseBodyError(#[source] hyper::http::Error),
    #[error("error stripping prefix from path")]
    StripPrefixError(#[source] std::path::StripPrefixError),
    #[error("unsupported method `{0}`")]
    UnsupportedMethod(hyper::http::Method),
}

pub(crate) async fn serve(options: ServerOptions) -> miette::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], options.port));

    let args = options.build_opts.clone();
    let destination = std::fs::canonicalize(&args.destination).into_diagnostic()?;

    let (send, mut recv) = tokio::sync::mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(move |result: Result<Event, _>| match result {
        Ok(event) => {
            debug!(?event);
            if event
                .paths
                .iter()
                .all(|path| path.starts_with(&destination))
            {
                debug!("Changed file is in output directory; skipping rebuild");
                return;
            }
            let result = send.blocking_send(GeneratorMessage::Rebuild);
            debug!(?result);
        }
        Err(e) => error!("{e}"),
    })
    .into_diagnostic()?;

    let path = std::fs::canonicalize(&find_site_root(options.build_opts.path.as_deref())?)
        .into_diagnostic()?;
    watcher
        .watch(&path, RecursiveMode::Recursive)
        .into_diagnostic()?;

    // FIXME: Watch for file changes and rebuild the site if it changes.
    let generate = tokio::spawn(async move {
        loop {
            let start = Instant::now();

            let site = match SiteIndex::from_directory(&path, options.build_opts.unpublished).await
            {
                Ok(site) => site,
                Err(e) => {
                    error!("failed to load site directory: {e}");
                    continue;
                }
            };

            let site = match site.render() {
                Ok(site) => site,
                Err(e) => {
                    error!("failed to render site: {e}");
                    continue;
                }
            };

            // FIXME: share this with the build code
            let gcx = GeneratorContext::new(&site, &args).unwrap();
            if let Err(e) = gcx.generate_site(&site).await {
                error!("failed to generate site: {e}");
                continue;
            }

            info!(
                "Generating site took {:.3} seconds",
                start.elapsed().as_secs_f32()
            );

            match recv.recv().await {
                Some(GeneratorMessage::Rebuild) => (),
                None => error!("error receiving message"),
            }
        }
    });

    // FIXME: we probably don't want to actually leak this...
    let serve_path = Box::leak(Box::new(options.build_opts.destination)).as_path();

    println!("Listening on http://{addr}");
    Server::bind(&addr)
        .serve(make_service_fn(
            |_conn: &hyper::server::conn::AddrStream| async move {
                Ok::<_, Infallible>(service_fn(move |req| async move {
                    match handle_request(req, serve_path).await {
                        Ok(response) => Ok(response),
                        Err(e) => generate_error_response(e).await,
                    }
                }))
            },
        ))
        .await
        .into_diagnostic()?;

    generate.await.into_diagnostic()?;

    Ok(())
}

async fn handle_request(req: Request<Body>, site: &Path) -> Result<Response<Body>, ServerError> {
    debug!(?req);

    let response = if req.method() == Method::GET {
        // FIXME: check the URI and find the right file to serve.
        let path = site.join(
            Path::new(req.uri().path())
                .strip_prefix("/")
                .map_err(ServerError::StripPrefixError)?,
        );
        debug!("checking if `{}` exists", path.display());
        if path.is_file() {
            serve_path(path.as_path()).await?
        } else {
            let path = path.join("index.html");
            if path.exists() {
                debug!("attempting to serve index path `{}`", path.display());
                serve_path(path.as_path()).await?
            } else {
                debug!("`{}` not found, returning 404", path.display());
                return Err(ServerError::PathNotFound(req.uri().clone()));
            }
        }
    } else {
        return Err(ServerError::UnsupportedMethod(req.method().clone()));
    };

    Ok(response)
}

async fn serve_path(path: &Path) -> Result<Response<Body>, ServerError> {
    let mut response = Response::builder();
    if let Some(mime) = guess_mime_type_from_path(path) {
        debug!("guessed mime type `{mime}`");
        response = response.header("Content-Type", mime);
    }
    let data = tokio::fs::read(path)
        .await
        .map_err(ServerError::ReadContents)?;
    debug!("writing {} bytes", data.len());
    response
        .header("Content-Length", data.len())
        .body(data.into())
        .map_err(ServerError::ResponseBodyError)
}

fn guess_mime_type_from_path(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "html" => Some("text/html"),
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        "ttf" => Some("font/ttf"),
        "woff2" => Some("font/woff2"),
        // FIXME: find a way to separate atom from a raw xml file
        "xml" => Some("application/atom+xml"),
        ext => {
            debug!("no known mime type for extension `{ext}`");
            None
        }
    }
}

async fn generate_error_response(e: ServerError) -> Result<Response<Body>, Infallible> {
    let body = format!("{e}");
    let status = match e {
        ServerError::PathNotFound(_) => StatusCode::NOT_FOUND,
        ServerError::ResponseBodyError(_)
        | ServerError::StripPrefixError(_)
        | ServerError::ReadContents(_) => StatusCode::INTERNAL_SERVER_ERROR,
        ServerError::UnsupportedMethod(_) => StatusCode::METHOD_NOT_ALLOWED,
    };
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .body(body.into())
        .unwrap())
}

#[cfg(test)]
mod test {
    use std::{
        io::BufRead,
        path::{Path, PathBuf},
    };

    use hyper::{body::to_bytes, Request, StatusCode};
    use miette::IntoDiagnostic;

    use crate::serve::{guess_mime_type_from_path, handle_request, ServerError};

    #[test]
    fn test_mime_type() {
        let path = Path::new("index.html");
        assert_eq!(guess_mime_type_from_path(path), Some("text/html"));
    }

    fn test_site() -> PathBuf {
        Path::new(".").join("test").join("data").join("html")
    }

    /// Make sure we can fetch a file that's known to exist
    #[tokio::test]
    async fn get_file() -> miette::Result<()> {
        let site = test_site();

        let req = Request::builder()
            .uri("/index.html")
            .body("".into())
            .into_diagnostic()?;

        let res = handle_request(req, &site).await.into_diagnostic()?;

        assert_eq!(res.status(), StatusCode::OK);

        let expected = "<!DOCTYPE html>
<html>

<body>
    Hello, World!
</body>

</html>";
        // Read the body but replace line endings to deal with platform differences.
        let body = to_bytes(res.into_body())
            .await
            .into_diagnostic()?
            .lines()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(body, expected);

        Ok(())
    }

    /// Make sure we can fetch the index of a directory
    #[tokio::test]
    async fn get_index() -> miette::Result<()> {
        let site = test_site();

        let req = Request::builder()
            .uri("/")
            .body("".into())
            .into_diagnostic()?;

        let res = handle_request(req, &site).await.into_diagnostic()?;

        assert_eq!(res.status(), StatusCode::OK);

        let expected = "<!DOCTYPE html>
<html>

<body>
    Hello, World!
</body>

</html>";
        // Read the body but replace line endings to deal with platform differences.
        let body = to_bytes(res.into_body())
            .await
            .into_diagnostic()?
            .lines()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(body, expected);

        Ok(())
    }

    /// Make sure we report an error if we ask for a nonexistent file
    #[tokio::test]
    async fn not_found() -> miette::Result<()> {
        let site = test_site();

        let req = Request::builder()
            .uri("/not-found")
            .body("".into())
            .into_diagnostic()?;

        let res = handle_request(req, &site).await;

        assert!(matches!(res, Err(ServerError::PathNotFound(_))));

        Ok(())
    }
}

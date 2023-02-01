use std::{convert::Infallible, net::SocketAddr, path::Path, sync::Arc};

use clap::Args;
use ebg::{
    generator::{generate_site, Options},
    site::Site,
};
use eyre::Context;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server,
};
use tracing::{debug, info};

#[derive(Args)]
pub struct ServerOptions {
    #[command(flatten)]
    build_opts: Options,

    #[clap(default_value_t = 4000)]
    port: u16,
}

pub(crate) async fn serve(options: ServerOptions) -> eyre::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], options.port));

    let args = options.build_opts.clone();

    let site = Arc::new(
        Site::from_directory(options.build_opts.path, options.build_opts.unpublished)
            .await
            .context("loading site content")?,
    );

    // FIXME: Watch for file changes and rebuild the site if it changes.
    let generator_site = site.clone();
    let generate = tokio::spawn(async move {
        // FIXME: share this with the build code
        generate_site(&generator_site, &args)
            .await
            .context("generating site")
            .unwrap();
    });

    // FIXME: we probably don't want to actually leak this...
    let serve_path = Box::leak(Box::new(options.build_opts.destination)).as_path();

    info!("Listening on {addr}");
    Server::bind(&addr)
        .serve(make_service_fn(|_conn| async move {
            Ok::<_, Infallible>(service_fn(move |req| async move {
                handle_request(req, serve_path).await
            }))
        }))
        .await?;

    generate.await?;

    Ok(())
}

async fn handle_request(req: Request<Body>, site: &Path) -> Result<Response<Body>, Infallible> {
    info!(?req);

    let response = if req.method() == Method::GET {
        // FIXME: check the URI and find the right file to serve.
        let path = site.join(Path::new(req.uri().path()).strip_prefix("/").unwrap());
        debug!("checking if `{}` exists", path.display());
        if path.is_file() {
            Response::new(tokio::fs::read(path).await.unwrap().into())
        } else {
            let path = path.join("index.html");
            Response::new(tokio::fs::read(path).await.unwrap().into())
        }
    } else {
        Response::new("Hello, World!".into())
    };

    Ok(response)
}

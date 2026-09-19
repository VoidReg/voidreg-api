use std::io;

use tracing::info;

use crate::app::build;
use crate::config::Config;
use crate::db::Db;

#[cfg(all(feature = "swagger", not(debug_assertions)))]
compile_error!("swagger is debug-only; build release with --no-default-features");

pub fn install_crypto() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

pub fn worker_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

pub async fn run(workers: usize) -> io::Result<()> {
    let config = Config::from_env().map_err(|err| io::Error::other(err.to_string()))?;
    let db = Db::connect(&config)
        .await
        .map_err(|err| io::Error::other(err.to_string()))?;
    db.seed_if_empty()
        .await
        .map_err(|err| io::Error::other(err.to_string()))?;

    let bind = format!("{}:{}", config.host, config.port);
    info!(
        host = %config.host,
        port = config.port,
        workers,
        "starting voidreg-api"
    );

    let server = actix_web::HttpServer::new(move || build(db.clone(), config.clone()))
        .workers(workers)
        .bind(&bind)?
        .run();

    let handle = server.handle();
    tokio::spawn(async move {
        shutdown_signal().await;
        info!("shutdown signal received");
        handle.stop(true).await;
    });

    server.await
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(signal) => signal,
                Err(err) => {
                    tracing::error!(error = %err, "failed to listen for SIGTERM");
                    ctrl_c.await.ok();
                    return;
                }
            };

        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}

pub mod api;
pub mod app;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod mail;
#[cfg(feature = "swagger")]
pub mod openapi;
pub mod telemetry;

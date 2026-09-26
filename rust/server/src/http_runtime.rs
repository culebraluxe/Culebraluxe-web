use crate::api::{build_application, error_capture, ApiConfig};
use db::Database;
use std::error::Error;
use std::future::IntoFuture;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

pub async fn run_http_server() -> Result<(), Box<dyn Error>> {
    crate::observability::init_tracing();
    let db = Database::connect_from_env().await?;
    let _ = db::shared::install(db.clone());
    let config = ApiConfig::from_env().map_err(std::io::Error::other)?;
    error_capture::install(db.clone());
    let infrastructure = crate::service_bootstrap::production_service_infrastructure(&db).await?;
    let (app, service_harness) = build_application(db.clone(), infrastructure, config);
    service_harness
        .start()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let keepalive = db.spawn_keepalive(&tokio::runtime::Handle::current());

    let bind = std::env::var("RUST_API_BIND")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("PORT")
                .ok()
                .map(|port| format!("0.0.0.0:{}", port.trim()))
        })
        .unwrap_or_else(|| "127.0.0.1:8080".into());

    std::thread::Builder::new()
        .name("engine-warmup".into())
        .spawn(|| match forge::engine::re_runtime::re_engine() {
            Ok(_) => tracing::info!(target: "culebraluxe::engine", "engine warmed"),
            Err(error) => {
                tracing::warn!(target: "culebraluxe::engine", %error, "engine warm-up deferred")
            }
        })
        .ok();

    let listener = TcpListener::bind(&bind).await?;
    tracing::info!(
        target: "culebraluxe::server",
        bind = %bind,
        database_target = %db.target().as_str(),
        "culebraluxe rust api listening"
    );

    let http_shutdown = CancellationToken::new();
    let signal_token = http_shutdown.clone();
    let signal_harness = service_harness.clone();
    let signal_task = tokio::spawn(async move {
        shutdown_signal().await;
        // Stop new background claims first. Existing HTTP handlers keep their
        // internal service access until Axum has drained them.
        signal_harness.quiesce_mq();
        signal_token.cancel();
    });
    let mut server = Box::pin(
        axum::serve(listener, app)
            .with_graceful_shutdown(http_shutdown.clone().cancelled_owned())
            .into_future(),
    );
    let serve_result = tokio::select! {
        result = &mut server => result.map_err(std::io::Error::other),
        _ = http_shutdown.cancelled() => {
            timeout(Duration::from_secs(30), &mut server)
                .await
                .map_err(|_| std::io::Error::other("HTTP drain exceeded its 30 second deadline"))?
                .map_err(std::io::Error::other)
        }
    };
    signal_task.abort();
    let _ = signal_task.await;

    let shutdown_result = service_harness
        .shutdown()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()));

    serve_result?;
    shutdown_result?;

    if let Some(keepalive) = keepalive {
        keepalive.abort();
        let _ = keepalive.await;
    }

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }

    tracing::info!(target: "culebraluxe::service::lifecycle", "service harness shutdown requested");
}

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
    // THE BOOT GATE — A NAMED REFUSAL BEFORE THE SOCKET, NOT A 500 AFTER IT (TST-DB-MIGRATION-007).
    //
    // `db::assert_boot_ready` reads the `schema_migration` ledger and refuses when a required migration is
    // unapplied on the database this process resolved (DEV or PROD — `db.target()` says which). Without it a
    // build pointed at a database that does not match it serves screens whose first query dies on a missing
    // column: an outage with no named cause. The refusal is a `DbFailure`, announced through `db::capture`
    // from its constructor, and the error sink was installed on the line above — so a refused boot leaves an
    // `app_error` row behind as well as a non-zero exit.
    //
    // BREAK-GLASS: `CULEBRALUXE_SKIP_BOOT_MIGRATION_GATE=1` starts anyway and says so in the log. It exists
    // because this gate runs inside a deploy — a false refusal takes production down, and an operator has to
    // be able to raise the site and then fix the ledger rather than wait on a code change to do either. Its
    // use is announced, never silent.
    if let Err(refusal) = db::assert_boot_ready(&db).await {
        let skipped = std::env::var("CULEBRALUXE_SKIP_BOOT_MIGRATION_GATE")
            .map(|value| value.trim() == "1")
            .unwrap_or(false);
        if !skipped {
            tracing::error!(
                target: "culebraluxe::server",
                database_target = %db.target().as_str(),
                %refusal,
                "refusing to serve: the database does not match this build"
            );
            return Err(refusal.into());
        }
        tracing::error!(
            target: "culebraluxe::server",
            database_target = %db.target().as_str(),
            %refusal,
            "boot migration gate REFUSED and was overridden by CULEBRALUXE_SKIP_BOOT_MIGRATION_GATE=1 — serving anyway"
        );
    }
    let infrastructure = crate::service_bootstrap::production_service_infrastructure(&db).await?;
    let (app, service_harness) = build_application(db.clone(), infrastructure, config);
    service_harness
        .start()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let keepalive = db.spawn_keepalive(&tokio::runtime::Handle::current());

    let bind = server_bind();

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

    tracing::info!(target: "culebraluxe::services::lifecycle", "service harness shutdown requested");
}

/// WHERE THIS SERVER LISTENS — the single place that decides it: `RUST_API_BIND`, else `PORT`, else the Mac's own
/// default. The socket and the signing-link origin both come from here.
///
/// THE SOCKET AND THE LINK ARE ONE FACT. A signing link is signed with the secret of the server that minted it, so
/// the link has to point back at that server. On 2026-10-11 a DEV server announced the production host instead, and
/// every invitation it mailed died on `SIGNER_ACCESS_INVALID` at the other end. Nothing may answer "where do I
/// listen" twice.
pub(crate) fn server_bind() -> String {
    resolve_server_bind(
        std::env::var("RUST_API_BIND").ok().as_deref(),
        std::env::var("PORT").ok().as_deref(),
    )
}

fn resolve_server_bind(rust_api_bind: Option<&str>, port: Option<&str>) -> String {
    rust_api_bind
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            port.map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|port| format!("0.0.0.0:{port}"))
        })
        .unwrap_or_else(|| "127.0.0.1:8080".into())
}

/// THE ORIGIN A BROWSER REACHES THIS SERVER ON, read off the address it listens on. `None` when the bind is not an
/// address a link can open, so the caller says what to use instead rather than mailing a link to nowhere.
///
/// A wildcard or loopback host is said as `localhost`: no browser opens `0.0.0.0`, and the person testing is at this
/// machine already. A LAN address is kept as it is, so a phone on the same network can sign too.
pub(crate) fn server_origin() -> Option<String> {
    origin_for_bind(&server_bind())
}

fn origin_for_bind(bind: &str) -> Option<String> {
    let bind = bind.trim();
    if bind.is_empty() || bind.contains(['/', '@']) || bind.chars().any(char::is_whitespace) {
        return None;
    }
    // `host:port`, `[::1]:port`, `:port`. A bare IPv6 address has more colons than a port split can read.
    let (host, port) = match bind.rsplit_once(':') {
        Some((host, port)) if bind.matches(':').count() == 1 || host.starts_with('[') => {
            (host, Some(port))
        }
        _ => (bind, None),
    };
    let port = port.filter(|port| !port.is_empty() && port.chars().all(|d| d.is_ascii_digit()));
    let host = match host.trim().trim_matches(|c| c == '[' || c == ']') {
        "" | "0.0.0.0" | "127.0.0.1" | "::" | "::1" | "localhost" => "localhost",
        other => other,
    };
    match port {
        Some(port) => Some(format!("http://{host}:{port}")),
        None => Some(format!("http://{host}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bind_is_declared_then_ported_then_defaulted() {
        assert_eq!(
            resolve_server_bind(Some(" 127.0.0.1:3000 "), Some("9000")),
            "127.0.0.1:3000"
        );
        assert_eq!(
            resolve_server_bind(Some("   "), Some("9000")),
            "0.0.0.0:9000"
        );
        assert_eq!(resolve_server_bind(None, Some("9000")), "0.0.0.0:9000");
        assert_eq!(resolve_server_bind(None, None), "127.0.0.1:8080");
    }

    #[test]
    fn a_local_bind_becomes_the_origin_a_browser_can_open() {
        assert_eq!(
            origin_for_bind("127.0.0.1:3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            origin_for_bind("0.0.0.0:8099").as_deref(),
            Some("http://localhost:8099")
        );
        assert_eq!(
            origin_for_bind("[::1]:3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(origin_for_bind("::1").as_deref(), Some("http://localhost"));
        assert_eq!(
            origin_for_bind(":3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            origin_for_bind("localhost:3000").as_deref(),
            Some("http://localhost:3000")
        );
        // A LAN address is kept: that link opens on the phone testing beside the Mac.
        assert_eq!(
            origin_for_bind("192.168.1.5:3000").as_deref(),
            Some("http://192.168.1.5:3000")
        );
        // A bind no browser can open is no origin at all: the caller decides what to say instead.
        assert_eq!(origin_for_bind("https://example.test"), None);
        assert_eq!(origin_for_bind(""), None);
    }
}

use db::Database;
use server::api::{build_application, error_capture, ApiConfig};
use server::security::{CasbinAuthorizationPort, DurableSecurityAuditPort};
use service::{CapturingDomainEventPort, ServiceInfrastructure};
use std::{error::Error, sync::Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db = Database::connect_from_env().await?;
    // Hand this pool to everything that cannot be constructed with one: the workflow engine's store, the Forge session
    // helper, the error capture sink. One pool per process instead of one per component, which is also what makes the
    // engine a part of this server rather than an island that happens to run inside it.
    let _ = db::shared::install(db.clone());
    let config = ApiConfig::from_env().map_err(std::io::Error::other)?;
    // Every database failure this process produces lands in app_error, alongside the TypeScript ones. Best effort and
    // recursion-guarded on the db side, so a dead database cannot fail this and cannot loop on itself.
    error_capture::install(db.clone());
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(CasbinAuthorizationPort::new().await?),
        Arc::new(DurableSecurityAuditPort::new(db::SecurityAuditDao::new(
            db.clone(),
        ))),
        Arc::new(CapturingDomainEventPort::default()),
    );
    let (app, service_kernel) = build_application(db.clone(), infrastructure, config);

    // The server is a long-lived process, so its pool is the one that stays warm. Neon suspends an idle database and a
    // suspended database turns the next user page load into a cold connect - the exact cost the retry policy exists to
    // limp through. A ping every few minutes means users never pay it. `FORGE_DB_KEEPALIVE_MS=0` turns it off.
    let _keepalive = db.spawn_keepalive(&tokio::runtime::Handle::current());

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
    // Warm the engine off the runtime.
    //
    // Building the engine is not per-call work any more, but the FIRST command after a process start pays for it:
    // parse the supermodel, validate it, seed it, connect. Measured at ~1.7s, once. Paying it at boot instead means a
    // user never pays it. It runs on a plain thread because the store's constructor blocks, and `block_on` from a
    // runtime thread panics - the exact failure this engine hit before.
    //
    // Best effort by design: if the warm-up fails (database not up yet, say), nothing is cached and the first real
    // command retries the build, which is how a failed build is supposed to behave.
    std::thread::Builder::new()
        .name("engine-warmup".into())
        .spawn(|| match forge::engine::re_runtime::re_engine() {
            Ok(_) => println!("culebraluxe rust api: engine warmed"),
            Err(error) => println!("culebraluxe rust api: engine warm-up deferred: {error}"),
        })
        .ok();

    let listener = TcpListener::bind(&bind).await?;

    println!(
        "culebraluxe rust api listening bind={} target={}",
        bind,
        db.target().as_str()
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(service_kernel.clone()))
        .await?;

    service_kernel
        .wait_stopped()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    if let Some(keepalive) = _keepalive {
        keepalive.abort();
        let _ = keepalive.await;
    }

    Ok(())
}

async fn shutdown_signal(service_kernel: server::ServiceKernel) {
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

    service_kernel.begin_shutdown();
}

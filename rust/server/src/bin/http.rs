use db::Database;
use server::api::{build_router, error_capture, ApiConfig};
use service::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceInfrastructure,
};
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
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    );
    let app = build_router(db.clone(), infrastructure, config);

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
    let listener = TcpListener::bind(&bind).await?;

    println!(
        "culebraluxe rust api listening bind={} target={}",
        bind,
        db.target().as_str()
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

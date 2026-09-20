use db::Database;
use server::api::{build_router, ApiConfig};
use service::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceInfrastructure,
};
use std::{error::Error, sync::Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db = Database::connect_from_env().await?;
    let config = ApiConfig::from_env().map_err(std::io::Error::other)?;
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    );
    let app = build_router(db.clone(), infrastructure, config);

    let bind = std::env::var("RUST_API_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
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

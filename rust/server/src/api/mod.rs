use crate::CoreServices;
use db::Database;
use service::ServiceInfrastructure;
use sha2::{Digest, Sha256};
use std::sync::Arc;

mod context;
mod error;
pub mod error_capture;
mod routes;

pub use error::ApiError;

#[derive(Clone)]
pub struct ApiConfig {
    pub internal_api_key: Arc<str>,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, String> {
        if let Ok(value) = std::env::var("CULEBRA_INTERNAL_API_KEY") {
            let trimmed = value.trim();
            if trimmed.len() >= 16 {
                return Ok(Self {
                    internal_api_key: Arc::from(trimmed),
                });
            }
        }

        let auth_secret = std::env::var("AUTH_SECRET")
            .map_err(|_| "CULEBRA_INTERNAL_API_KEY or AUTH_SECRET is required".to_owned())?;
        let auth_secret = auth_secret.trim();
        if auth_secret.len() < 16 {
            return Err(
                "AUTH_SECRET must be at least 16 characters for bridge-key derivation".into(),
            );
        }

        let mut digest = Sha256::new();
        digest.update(b"culebraluxe-rust-bridge:v1:");
        digest.update(auth_secret.as_bytes());
        let internal_api_key = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        Ok(Self {
            internal_api_key: Arc::from(internal_api_key),
        })
    }
}

#[derive(Clone)]
pub struct ApiState {
    db: Database,
    infrastructure: ServiceInfrastructure,
    config: ApiConfig,
}

impl ApiState {
    pub fn new(db: Database, infrastructure: ServiceInfrastructure, config: ApiConfig) -> Self {
        Self {
            db,
            infrastructure,
            config,
        }
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn services(&self) -> CoreServices {
        CoreServices::new(self.db.clone(), self.infrastructure.clone())
    }

    fn internal_api_key(&self) -> &str {
        &self.config.internal_api_key
    }
}

pub fn build_router(
    db: Database,
    infrastructure: ServiceInfrastructure,
    config: ApiConfig,
) -> axum::Router {
    routes::router(ApiState::new(db, infrastructure, config))
}

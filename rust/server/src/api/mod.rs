use crate::CoreServices;
use db::Database;
use service::ServiceInfrastructure;
use std::sync::Arc;

mod context;
mod error;
mod routes;

pub use error::ApiError;

#[derive(Clone)]
pub struct ApiConfig {
    pub internal_api_key: Arc<str>,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, String> {
        let value = std::env::var("CULEBRA_INTERNAL_API_KEY")
            .map_err(|_| "CULEBRA_INTERNAL_API_KEY is required".to_owned())?;
        let trimmed = value.trim();
        if trimmed.len() < 16 {
            return Err("CULEBRA_INTERNAL_API_KEY must be at least 16 characters".into());
        }
        Ok(Self {
            internal_api_key: Arc::from(trimmed),
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

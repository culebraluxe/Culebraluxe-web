use crate::firms::FirmService;
use crate::people::PersonService;
use crate::properties::PropertyService;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use db::{Database, FirmDao, PersonDao, PropertyDao};
use service::{ServiceContext, ServiceInfrastructure};

#[async_trait]
pub trait CoreEntityLookup: Send + Sync {
    async fn person_exists(
        &self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError>;

    async fn firm_exists(
        &self,
        firm_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError>;

    async fn property_exists(
        &self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError>;
}

#[derive(Clone)]
pub struct ServiceDirectory {
    db: Database,
    infrastructure: ServiceInfrastructure,
}

impl ServiceDirectory {
    pub fn new(db: Database, infrastructure: ServiceInfrastructure) -> Self {
        Self { db, infrastructure }
    }
}

#[async_trait]
impl CoreEntityLookup for ServiceDirectory {
    async fn person_exists(
        &self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        let mut service =
            PersonService::new(PersonDao::new(self.db.clone()), self.infrastructure.clone());
        Ok(service.get(person_id, context).await?.is_some())
    }

    async fn firm_exists(
        &self,
        firm_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        let mut service =
            FirmService::new(FirmDao::new(self.db.clone()), self.infrastructure.clone());
        Ok(service.get(firm_id, context).await?.is_some())
    }

    async fn property_exists(
        &self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        let mut service =
            PropertyService::new(PropertyDao::new(self.db.clone()), self.infrastructure.clone());
        Ok(service.get(property_id, context).await?.is_some())
    }
}

use crate::firms::FirmService;
use crate::people::PersonService;
use crate::properties::PropertyService;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use db::{FirmDao, PersonDao, PropertyDao};
use service::ServiceContext;
use std::sync::Arc;

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
    person: Arc<PersonService<PersonDao>>,
    firm: Arc<FirmService<FirmDao>>,
    property: Arc<PropertyService<PropertyDao>>,
}

impl ServiceDirectory {
    pub fn new(
        person: Arc<PersonService<PersonDao>>,
        firm: Arc<FirmService<FirmDao>>,
        property: Arc<PropertyService<PropertyDao>>,
    ) -> Self {
        Self {
            person,
            firm,
            property,
        }
    }
}

#[async_trait]
impl CoreEntityLookup for ServiceDirectory {
    async fn person_exists(
        &self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        Ok(self.person.get(person_id, context).await?.is_some())
    }

    async fn firm_exists(
        &self,
        firm_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        Ok(self.firm.get(firm_id, context).await?.is_some())
    }

    async fn property_exists(
        &self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        Ok(self.property.get(property_id, context).await?.is_some())
    }
}

use crate::communications::CommsService;
use crate::contracts::ContractService;
use crate::firms::FirmService;
use crate::lookup::ServiceDirectory;
use crate::people::PersonService;
use crate::projects::ProjectService;
use crate::properties::PropertyService;
use crate::security::SecurityService;
use crate::showings::ShowingService;
use crate::wbs::WbsService;
use db::{
    CommsDao, ContractDao, Database, FirmDao, PersonDao, ProjectDao, PropertyDao, SecurityDao, ShowingDao,
    WbsDao,
};
use service::ServiceInfrastructure;
use std::sync::Arc;

/// Rust composition root for the required CulebraLuxe business-service kernel.
///
/// The services are deliberately lightweight factories over one shared Database
/// pool and one shared infrastructure bundle. Cross-domain existence checks use
/// ServiceDirectory, which calls the owning service rather than another
/// domain's repository.
#[derive(Clone)]
pub struct CoreServices {
    db: Database,
    infrastructure: ServiceInfrastructure,
    directory: ServiceDirectory,
}

impl CoreServices {
    pub fn new(db: Database, infrastructure: ServiceInfrastructure) -> Self {
        let directory = ServiceDirectory::new(db.clone(), infrastructure.clone());
        Self {
            db,
            infrastructure,
            directory,
        }
    }

    pub fn person(&self) -> PersonService<PersonDao> {
        PersonService::new(PersonDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn firm(&self) -> FirmService<FirmDao> {
        FirmService::new(FirmDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn property(&self) -> PropertyService<PropertyDao> {
        PropertyService::new(
            PropertyDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn comms(&self) -> CommsService<CommsDao> {
        CommsService::new(CommsDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn contract(&self) -> ContractService<ContractDao> {
        ContractService::new(
            ContractDao::new(self.db.clone()),
            Arc::new(self.directory.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn showing(&self) -> ShowingService<ShowingDao> {
        ShowingService::new(
            ShowingDao::new(self.db.clone()),
            Arc::new(self.directory.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn security(&self) -> SecurityService<SecurityDao> {
        SecurityService::new(
            SecurityDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn wbs(&self) -> WbsService<WbsDao> {
        WbsService::new(WbsDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn project(&self) -> ProjectService<ProjectDao> {
        ProjectService::new(
            ProjectDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }
}

use crate::accounting::AccountingService;
use crate::calendar::CalendarService;
use crate::clients::ClientService;
use crate::cockpit::CockpitService;
use crate::communications::CommsService;
use crate::contracts::ContractService;
use crate::deals::DealPortalService;
use crate::firms::FirmService;
use crate::flight_recorder::FlightRecorderService;
use crate::forms::FormService;
use crate::guide::GuideService;
use crate::lookup::ServiceDirectory;
use crate::media::MediaService;
use crate::people::PersonService;
use crate::projects::ProjectService;
use crate::properties::PropertyService;
use crate::public_listings::PublicListingService;
use crate::security::SecurityService;
use crate::showings::ShowingService;
use crate::signature::SignatureService;
use crate::task::TaskService;
use crate::vault::{VaultArtifactPort, VaultService};
use crate::wbs::WbsService;
use crate::website_leads::WebsiteLeadService;
use crate::workflow_portal::WorkflowPortalService;
use db::{
    AccountingDao, CalendarDao, ClientDao, CockpitDao, CommsDao, ContractDao, Database,
    DealPortalDao, FirmDao, FlightRecorderDao, FormDao, GuideDao, MediaDao, PersonDao, ProjectDao,
    PropertyDao, SecurityDao, ShowingDao, SignatureDao, TaskDao, VaultDao, WbsDao,
    WorkflowPortalDao,
};
use service::{ServiceInfrastructure, SignatureProvider};
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

    pub fn clients(&self) -> ClientService<ClientDao> {
        ClientService::new(ClientDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn cockpit(&self) -> CockpitService<CockpitDao> {
        CockpitService::new(
            CockpitDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn guide(&self) -> GuideService<GuideDao> {
        GuideService::new(GuideDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn media(&self) -> MediaService<MediaDao> {
        MediaService::new(MediaDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn person(&self) -> PersonService<PersonDao> {
        PersonService::new(PersonDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn firm(&self) -> FirmService<FirmDao> {
        FirmService::new(FirmDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn forms(&self) -> FormService<FormDao> {
        FormService::new(FormDao::new(self.db.clone()), self.infrastructure.clone())
    }

    /// The public site's listing copy, authorized as the published `property.public.read`.
    pub fn public_listings(&self) -> PublicListingService<db::PublicListingDao> {
        PublicListingService::new(
            db::PublicListingDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    /// External guests: emailed sign-in codes and provisioning on first sign-in. Mail as for lead emails.
    pub fn guest_sign_in(&self) -> crate::security::GuestSignInService<db::GuestDao> {
        crate::security::GuestSignInService::new(
            db::GuestDao::new(self.db.clone()),
            crate::security::guest_mail_from_env(),
            self.infrastructure.clone(),
        )
    }

    /// A website lead's emails, sent with the mail settings in the environment (none: the service refuses).
    pub fn website_leads(&self) -> WebsiteLeadService<db::WebsiteLeadDao> {
        WebsiteLeadService::new(
            db::WebsiteLeadDao::new(self.db.clone()),
            crate::website_leads::mail_from_env(),
            self.infrastructure.clone(),
        )
    }

    pub fn property(&self) -> PropertyService<PropertyDao> {
        PropertyService::new(
            PropertyDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn calendar(&self) -> CalendarService<CalendarDao> {
        CalendarService::new(
            CalendarDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn comms(&self) -> CommsService<CommsDao> {
        CommsService::new(CommsDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn deal_portal(&self) -> DealPortalService<DealPortalDao> {
        DealPortalService::new(
            DealPortalDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
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

    pub fn signature(
        &self,
        provider: Arc<dyn SignatureProvider>,
    ) -> SignatureService<SignatureDao> {
        SignatureService::new(
            SignatureDao::new(self.db.clone()),
            provider,
            self.infrastructure.clone(),
        )
    }

    pub fn security(&self) -> SecurityService<SecurityDao> {
        SecurityService::new(
            SecurityDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn vault(&self, artifacts: Arc<dyn VaultArtifactPort>) -> VaultService<VaultDao> {
        VaultService::new(
            VaultDao::new(self.db.clone()),
            artifacts,
            self.infrastructure.clone(),
        )
    }

    pub fn task(&self) -> TaskService<TaskDao> {
        TaskService::new(TaskDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn wbs(&self) -> WbsService<WbsDao> {
        WbsService::new(WbsDao::new(self.db.clone()), self.infrastructure.clone())
    }

    pub fn workflow_portal(&self) -> WorkflowPortalService<WorkflowPortalDao> {
        WorkflowPortalService::new(
            WorkflowPortalDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn flight_recorder(&self) -> FlightRecorderService<FlightRecorderDao> {
        FlightRecorderService::new(
            FlightRecorderDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn project(&self) -> ProjectService<ProjectDao> {
        ProjectService::new(
            ProjectDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }

    pub fn accounting(&self) -> AccountingService<AccountingDao> {
        AccountingService::new(
            AccountingDao::new(self.db.clone()),
            self.infrastructure.clone(),
        )
    }
}

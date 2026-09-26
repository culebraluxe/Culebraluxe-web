use crate::{
    accounting::AccountingService,
    calendar::CalendarService,
    clients::ClientService,
    cockpit::CockpitService,
    communications::CommsService,
    contracts::ContractService,
    deals::DealPortalService,
    firms::FirmService,
    flight_recorder::FlightRecorderService,
    forms::FormService,
    guide::GuideService,
    intake::IntakeService,
    issues::IssueService,
    lookup::ServiceDirectory,
    marketing::MarketingService,
    media::MediaService,
    people::PersonService,
    projects::ProjectService,
    properties::PropertyService,
    public_listings::PublicListingService,
    relationship_evidence::RelationshipEvidenceService,
    security::{GuestSignInService, SecurityService},
    showings::ShowingService,
    signature::SignatureService,
    support::SupportDiagnosticsService,
    task::TaskService,
    tech::TechCockpitService,
    vault::{VaultArtifactPort, VaultService},
    wbs::WbsService,
    website_leads::WebsiteLeadService,
    whatsapp::WhatsAppService,
    workflow_portal::WorkflowPortalService,
};
use async_trait::async_trait;
use db::{
    AccountingDao, CalendarDao, ClientDao, CockpitDao, CommsDao, ContractDao, Database,
    DealPortalDao, FirmDao, FlightRecorderDao, FormDao, GuestDao, GuideDao, IntakeDao, IssueDao,
    MarketingDao, MediaDao, PersonDao, ProjectDao, PropertyDao, PublicListingDao,
    RelationshipEvidenceDao, SecurityDao, ShowingDao, SignatureDao, SupportDiagnosticsDao, TaskDao,
    TechCockpitDao, VaultDao, WbsDao, WebsiteLeadDao, WhatsAppDao, WorkflowPortalDao,
};
use domain::{
    VaultArtifactFailure, VaultCommandOutcome, VaultRenderRequest, VaultRenderedArtifact,
};
use integrations::boldsign::{BoldSignConfig, BoldSignSignatureProvider};
use service::{
    AbstractService, ServiceContext, ServiceDescriptor, ServiceDispatchError, ServiceEnvelope,
    ServiceInfrastructure,
};
use std::sync::Arc;
use tokio::sync::Mutex;

struct UnavailableVaultArtifactPort;

#[async_trait]
impl VaultArtifactPort for UnavailableVaultArtifactPort {
    async fn render_issued_document(
        &self,
        _request: VaultRenderRequest,
    ) -> Result<VaultRenderedArtifact, VaultArtifactFailure> {
        Err(VaultArtifactFailure {
            outcome: VaultCommandOutcome::PreconditionFailure,
            message: "Vault artifact renderer is not configured on this transport.".into(),
        })
    }
}

type SignatureCatalogEntry = Result<Arc<SignatureService<SignatureDao>>, Arc<str>>;

struct ProjectServiceHost {
    service: Arc<Mutex<ProjectService<ProjectDao>>>,
}

impl ProjectServiceHost {
    fn new(service: ProjectService<ProjectDao>) -> Self {
        Self {
            service: Arc::new(Mutex::new(service)),
        }
    }

    fn service(&self) -> Arc<Mutex<ProjectService<ProjectDao>>> {
        self.service.clone()
    }
}

macro_rules! abstract_service {
    ($service:ty, $domain:literal, $description:literal) => {
        #[async_trait]
        impl AbstractService for $service {
            fn descriptor(&self) -> ServiceDescriptor {
                ServiceDescriptor {
                    domain: $domain.into(),
                    version: "1".into(),
                    description: $description.into(),
                    capabilities: Vec::new(),
                    dependencies: Vec::new(),
                    invariants: Vec::new(),
                }
            }
        }
    };
}

abstract_service!(CockpitService<CockpitDao>, "cockpit", "Cockpit service");
abstract_service!(GuideService<GuideDao>, "guide", "Island guide service");
abstract_service!(IntakeService<IntakeDao>, "intake", "Website intake service");
abstract_service!(IssueService<IssueDao>, "issue", "Issue service");
abstract_service!(
    MarketingService<MarketingDao>,
    "marketing",
    "Marketing service"
);
abstract_service!(MediaService<MediaDao>, "media", "Media service");
abstract_service!(FormService<FormDao>, "forms", "Forms service");
abstract_service!(
    PublicListingService<PublicListingDao>,
    "public-listing",
    "Public listing service"
);
abstract_service!(
    GuestSignInService<GuestDao>,
    "guest-sign-in",
    "Guest sign-in service"
);
abstract_service!(
    WebsiteLeadService<WebsiteLeadDao>,
    "website-lead",
    "Website lead service"
);
abstract_service!(
    RelationshipEvidenceService<RelationshipEvidenceDao>,
    "relationship-evidence",
    "Relationship evidence service"
);
abstract_service!(CalendarService<CalendarDao>, "calendar", "Calendar service");
#[async_trait]
impl AbstractService for CommsService<CommsDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "communications".into(),
            version: "1".into(),
            description: "Communications service".into(),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            invariants: vec!["Communication reads use the startup-warmed service cache.".into()],
        }
    }

    async fn warm_cache(&self) -> Result<usize, ServiceDispatchError> {
        self.warm_read_cache().await.map_err(|error| {
            ServiceDispatchError::infrastructure(
                "DATABASE",
                format!("communications cache warm failed: {error}"),
                true,
            )
        })
    }
}
abstract_service!(DealPortalService<DealPortalDao>, "deal", "Deal service");
abstract_service!(ShowingService<ShowingDao>, "showing", "Showing service");
abstract_service!(
    SignatureService<SignatureDao>,
    "signature",
    "Electronic signature service"
);
abstract_service!(
    SupportDiagnosticsService<SupportDiagnosticsDao>,
    "support",
    "Support diagnostics service"
);
abstract_service!(VaultService<VaultDao>, "vault", "Document vault service");
abstract_service!(TaskService<TaskDao>, "task", "Task service");
abstract_service!(WbsService<WbsDao>, "wbs", "Work breakdown service");
abstract_service!(
    WorkflowPortalService<WorkflowPortalDao>,
    "workflow-portal",
    "Workflow portal service"
);
abstract_service!(
    FlightRecorderService<FlightRecorderDao>,
    "flight-recorder",
    "Flight recorder service"
);
abstract_service!(TechCockpitService<TechCockpitDao>, "tech", "Tech service");
abstract_service!(WhatsAppService<WhatsAppDao>, "whatsapp", "WhatsApp service");
abstract_service!(
    AccountingService<AccountingDao>,
    "accounting",
    "Accounting service"
);

#[async_trait]
impl AbstractService for ProjectServiceHost {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "project".into(),
            version: "1".into(),
            description: "Project service".into(),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            invariants: Vec::new(),
        }
    }
}

struct UnavailableService {
    domain: &'static str,
    reason: Arc<str>,
}

#[async_trait]
impl AbstractService for UnavailableService {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: self.domain.into(),
            version: "1".into(),
            description: format!("Unavailable service: {}", self.reason),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            invariants: Vec::new(),
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        _context: &ServiceContext,
    ) -> Result<serde_json::Value, ServiceDispatchError> {
        Err(ServiceDispatchError::infrastructure(
            "SERVICE_UNAVAILABLE",
            format!(
                "{}.{} is unavailable: {}",
                self.domain, envelope.operation, self.reason
            ),
            false,
        ))
    }
}

/// Long-lived, typed ownership for every route-facing business service.
///
/// Services are constructed once with the application infrastructure. Project is the
/// only locked entry because its transaction repository requires exclusive access.
#[derive(Clone)]
pub struct ServiceCatalog {
    clients: Arc<ClientService<ClientDao>>,
    cockpit: Arc<CockpitService<CockpitDao>>,
    guide: Arc<GuideService<GuideDao>>,
    intake: Arc<IntakeService<IntakeDao>>,
    issues: Arc<IssueService<IssueDao>>,
    marketing: Arc<MarketingService<MarketingDao>>,
    media: Arc<MediaService<MediaDao>>,
    person: Arc<PersonService<PersonDao>>,
    firm: Arc<FirmService<FirmDao>>,
    forms: Arc<FormService<FormDao>>,
    public_listings: Arc<PublicListingService<PublicListingDao>>,
    guest_sign_in: Arc<GuestSignInService<GuestDao>>,
    website_leads: Arc<WebsiteLeadService<WebsiteLeadDao>>,
    relationship_evidence: Arc<RelationshipEvidenceService<RelationshipEvidenceDao>>,
    property: Arc<PropertyService<PropertyDao>>,
    calendar: Arc<CalendarService<CalendarDao>>,
    comms: Arc<CommsService<CommsDao>>,
    deal_portal: Arc<DealPortalService<DealPortalDao>>,
    contract: Arc<ContractService<ContractDao>>,
    showing: Arc<ShowingService<ShowingDao>>,
    signature: SignatureCatalogEntry,
    support: Arc<SupportDiagnosticsService<SupportDiagnosticsDao>>,
    security: Arc<SecurityService<SecurityDao>>,
    vault: Arc<VaultService<VaultDao>>,
    task: Arc<TaskService<TaskDao>>,
    wbs: Arc<WbsService<WbsDao>>,
    workflow_portal: Arc<WorkflowPortalService<WorkflowPortalDao>>,
    flight_recorder: Arc<FlightRecorderService<FlightRecorderDao>>,
    project: Arc<ProjectServiceHost>,
    tech: Arc<TechCockpitService<TechCockpitDao>>,
    whatsapp: Arc<WhatsAppService<WhatsAppDao>>,
    accounting: Arc<AccountingService<AccountingDao>>,
}

impl ServiceCatalog {
    pub fn new(db: Database, infrastructure: ServiceInfrastructure) -> Self {
        let person = Arc::new(PersonService::new(
            PersonDao::new(db.clone()),
            infrastructure.clone(),
        ));
        let firm = Arc::new(FirmService::new(
            FirmDao::new(db.clone()),
            infrastructure.clone(),
        ));
        let property = Arc::new(PropertyService::new(
            PropertyDao::new(db.clone()),
            infrastructure.clone(),
        ));
        let directory = Arc::new(ServiceDirectory::new(
            person.clone(),
            firm.clone(),
            property.clone(),
        ));
        let signature = BoldSignConfig::from_env()
            .and_then(|config| BoldSignSignatureProvider::new(db.clone(), config))
            .map(|provider| {
                Arc::new(SignatureService::new(
                    SignatureDao::new(db.clone()),
                    Arc::new(provider),
                    infrastructure.clone(),
                ))
            })
            .map_err(Arc::<str>::from);
        Self {
            clients: Arc::new(ClientService::new(
                ClientDao::new(db.clone()),
                infrastructure.clone(),
            )),
            cockpit: Arc::new(CockpitService::new(
                CockpitDao::new(db.clone()),
                infrastructure.clone(),
            )),
            guide: Arc::new(GuideService::new(
                GuideDao::new(db.clone()),
                infrastructure.clone(),
            )),
            intake: Arc::new(IntakeService::new(
                IntakeDao::new(db.clone()),
                infrastructure.clone(),
            )),
            issues: Arc::new(IssueService::new(
                IssueDao::new(db.clone()),
                infrastructure.clone(),
            )),
            marketing: Arc::new(MarketingService::new(
                MarketingDao::new(db.clone()),
                infrastructure.clone(),
            )),
            media: Arc::new(MediaService::new(
                MediaDao::new(db.clone()),
                infrastructure.clone(),
            )),
            person,
            firm,
            forms: Arc::new(FormService::new(
                FormDao::new(db.clone()),
                infrastructure.clone(),
            )),
            public_listings: Arc::new(PublicListingService::new(
                PublicListingDao::new(db.clone()),
                infrastructure.clone(),
            )),
            guest_sign_in: Arc::new(GuestSignInService::new(
                GuestDao::new(db.clone()),
                crate::security::guest_mail_from_env(),
                infrastructure.clone(),
            )),
            website_leads: Arc::new(WebsiteLeadService::new(
                WebsiteLeadDao::new(db.clone()),
                crate::website_leads::mail_from_env(),
                infrastructure.clone(),
            )),
            relationship_evidence: Arc::new(RelationshipEvidenceService::new(
                RelationshipEvidenceDao::new(db.clone()),
                infrastructure.clone(),
            )),
            property,
            calendar: Arc::new(CalendarService::new(
                CalendarDao::new(db.clone()),
                infrastructure.clone(),
            )),
            comms: Arc::new(CommsService::new(
                CommsDao::new(db.clone()),
                infrastructure.clone(),
            )),
            deal_portal: Arc::new(DealPortalService::new(
                DealPortalDao::new(db.clone()),
                infrastructure.clone(),
            )),
            contract: Arc::new(ContractService::new(
                ContractDao::new(db.clone()),
                directory.clone(),
                infrastructure.clone(),
            )),
            showing: Arc::new(ShowingService::new(
                ShowingDao::new(db.clone()),
                directory,
                infrastructure.clone(),
            )),
            signature,
            support: Arc::new(SupportDiagnosticsService::new(
                SupportDiagnosticsDao::new(db.clone()),
                infrastructure.clone(),
            )),
            security: Arc::new(SecurityService::new(
                SecurityDao::new(db.clone()),
                infrastructure.clone(),
            )),
            vault: Arc::new(VaultService::new(
                VaultDao::new(db.clone()),
                Arc::new(UnavailableVaultArtifactPort),
                infrastructure.clone(),
            )),
            task: Arc::new(TaskService::new(
                TaskDao::new(db.clone()),
                infrastructure.clone(),
            )),
            wbs: Arc::new(WbsService::new(
                WbsDao::new(db.clone()),
                infrastructure.clone(),
            )),
            workflow_portal: Arc::new(WorkflowPortalService::new(
                WorkflowPortalDao::new(db.clone()),
                infrastructure.clone(),
            )),
            flight_recorder: Arc::new(FlightRecorderService::new(
                FlightRecorderDao::new(db.clone()),
                infrastructure.clone(),
            )),
            project: Arc::new(ProjectServiceHost::new(ProjectService::new(
                ProjectDao::new(db.clone()),
                infrastructure.clone(),
            ))),
            tech: Arc::new(TechCockpitService::new(
                TechCockpitDao::new(db.clone()),
                infrastructure.clone(),
            )),
            whatsapp: Arc::new(WhatsAppService::new(WhatsAppDao::new(db.clone()))),
            accounting: Arc::new(AccountingService::new(
                AccountingDao::new(db),
                infrastructure,
            )),
        }
    }

    pub fn signature(&self) -> Result<Arc<SignatureService<SignatureDao>>, Arc<str>> {
        self.signature.clone()
    }

    pub(crate) fn registrations(&self) -> Vec<Arc<dyn AbstractService>> {
        let mut services: Vec<Arc<dyn AbstractService>> = vec![
            self.person.clone(),
            self.firm.clone(),
            self.property.clone(),
            self.contract.clone(),
            self.clients.clone(),
            self.cockpit.clone(),
            self.guide.clone(),
            self.intake.clone(),
            self.issues.clone(),
            self.marketing.clone(),
            self.media.clone(),
            self.forms.clone(),
            self.public_listings.clone(),
            self.guest_sign_in.clone(),
            self.website_leads.clone(),
            self.relationship_evidence.clone(),
            self.calendar.clone(),
            self.comms.clone(),
            self.deal_portal.clone(),
            self.showing.clone(),
            self.support.clone(),
            self.security.clone(),
            self.vault.clone(),
            self.task.clone(),
            self.wbs.clone(),
            self.workflow_portal.clone(),
            self.flight_recorder.clone(),
            self.project.clone(),
            self.tech.clone(),
            self.whatsapp.clone(),
            self.accounting.clone(),
        ];
        match &self.signature {
            Ok(signature) => services.push(signature.clone()),
            Err(reason) => services.push(Arc::new(UnavailableService {
                domain: "signature",
                reason: reason.clone(),
            })),
        }
        services
    }
}

macro_rules! catalog_accessors {
    ($($name:ident: $ty:ty),+ $(,)?) => {$(
        impl ServiceCatalog { pub fn $name(&self) -> Arc<$ty> { self.$name.clone() } }
    )+};
}

catalog_accessors! {
    clients: ClientService<ClientDao>, cockpit: CockpitService<CockpitDao>, guide: GuideService<GuideDao>,
    intake: IntakeService<IntakeDao>, issues: IssueService<IssueDao>, marketing: MarketingService<MarketingDao>,
    media: MediaService<MediaDao>, person: PersonService<PersonDao>, firm: FirmService<FirmDao>,
    forms: FormService<FormDao>, public_listings: PublicListingService<PublicListingDao>,
    guest_sign_in: GuestSignInService<GuestDao>, website_leads: WebsiteLeadService<WebsiteLeadDao>,
    relationship_evidence: RelationshipEvidenceService<RelationshipEvidenceDao>, property: PropertyService<PropertyDao>,
    calendar: CalendarService<CalendarDao>, comms: CommsService<CommsDao>, deal_portal: DealPortalService<DealPortalDao>,
    contract: ContractService<ContractDao>, showing: ShowingService<ShowingDao>, support: SupportDiagnosticsService<SupportDiagnosticsDao>,
    security: SecurityService<SecurityDao>, vault: VaultService<VaultDao>, task: TaskService<TaskDao>, wbs: WbsService<WbsDao>,
    workflow_portal: WorkflowPortalService<WorkflowPortalDao>, flight_recorder: FlightRecorderService<FlightRecorderDao>,
    tech: TechCockpitService<TechCockpitDao>, whatsapp: WhatsAppService<WhatsAppDao>, accounting: AccountingService<AccountingDao>,
}

impl ServiceCatalog {
    pub fn project(&self) -> Arc<Mutex<ProjectService<ProjectDao>>> {
        self.project.service()
    }
}

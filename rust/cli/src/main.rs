use db::{Database, DbTarget, ProjectDao, ProjectTxDao};
use domain::{CreateProjectRequest, ProjectStatus, UpdateProjectRequest, WbsCategory};
use server::projects::ProjectService;
use service::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use std::error::Error;
use std::io;
use std::process::ExitCode;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();

    let result = match command.as_str() {
        "db-smoke" => db_smoke().await,
        "tx-smoke" => tx_smoke().await,
        _ => {
            eprintln!("usage: cargo run -p cli -- <db-smoke|tx-smoke>");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rust smoke failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn smoke_context(correlation_id: impl Into<String>) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("rust-smoke".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "rust-smoke".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["SYSTEM_SMOKE".into()],
        }),
    }
}

fn smoke_infrastructure() -> (
    ServiceInfrastructure,
    CapturingAuditPort,
    CapturingDomainEventPort,
) {
    let audit = CapturingAuditPort::default();
    let events = CapturingDomainEventPort::default();
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(audit.clone()),
        Arc::new(events.clone()),
    );
    (infrastructure, audit, events)
}

async fn db_smoke() -> Result<(), Box<dyn Error>> {
    let db = Database::connect_from_env().await?;
    db.ping().await?;

    let target = db.target();
    let (infrastructure, _, _) = smoke_infrastructure();
    let context = smoke_context("rust-db-smoke");
    let mut service = ProjectService::new(ProjectDao::new(db), infrastructure);
    let projects = service.list(&context).await?;

    println!(
        "rust db/service smoke ok target={} projects={}",
        target.as_str(),
        projects.len()
    );

    if let Some(project) = projects.first() {
        println!(
            "project sample id={} status={} areas={}",
            project.id,
            project.status,
            project.areas.len()
        );
    }

    Ok(())
}

async fn tx_smoke() -> Result<(), Box<dyn Error>> {
    let db = Database::connect_from_env().await?;
    if db.target() != DbTarget::Dev {
        return Err(io::Error::other("tx-smoke refuses every database target except DEV").into());
    }
    db.ping().await?;

    let id = format!("rust-slice2-{}", Uuid::new_v4());
    let context = smoke_context(format!("rust-tx-smoke-{id}"));
    let (infrastructure, audit, events) = smoke_infrastructure();

    let mut transaction = db.begin("project.tx-smoke").await?;
    {
        let dao = ProjectTxDao::new(&mut transaction);
        let mut service = ProjectService::new(dao, infrastructure);

        let created = service
            .create(
                &CreateProjectRequest {
                    id: id.clone(),
                    name: "Rust Slice 2 Transaction Probe".into(),
                    owner: Some("rust-smoke".into()),
                    description: "Created inside a DEV transaction and always rolled back.".into(),
                    areas: vec![WbsCategory::Management],
                    starts_at: None,
                    ends_at: None,
                    project_type: Some("rust-smoke".into()),
                    playbook_id: None,
                    playbook_version: None,
                    person_id: None,
                    property_id: None,
                    contract_id: None,
                },
                &context,
            )
            .await?;

        if created.status != ProjectStatus::Open {
            return Err(io::Error::other("created transaction probe was not open").into());
        }

        let inside = service
            .get(&id, &context)
            .await?
            .ok_or_else(|| io::Error::other("created transaction probe could not be read back"))?;
        if inside.id != id {
            return Err(io::Error::other("transaction read returned the wrong project").into());
        }

        let updated = service
            .update(
                &UpdateProjectRequest {
                    id: id.clone(),
                    name: None,
                    owner: None,
                    status: Some(ProjectStatus::Doing),
                    description: None,
                    areas: None,
                    starts_at: None,
                    ends_at: None,
                    project_type: None,
                    playbook_id: None,
                    playbook_version: None,
                    person_id: None,
                    property_id: None,
                    contract_id: None,
                },
                &context,
            )
            .await?;

        if updated.status != ProjectStatus::Doing {
            return Err(io::Error::other("transaction update did not reach doing").into());
        }
    }

    let audit_count = audit.events().len();
    let event_count = events.events().len();
    if audit_count != 3 {
        return Err(io::Error::other(format!(
            "expected 3 audit events inside transaction, got {audit_count}"
        ))
        .into());
    }
    if event_count != 2 {
        return Err(io::Error::other(format!(
            "expected 2 domain events inside transaction, got {event_count}"
        ))
        .into());
    }

    transaction.rollback().await?;

    let (verify_infrastructure, _, _) = smoke_infrastructure();
    let mut verifier = ProjectService::new(ProjectDao::new(db), verify_infrastructure);
    if verifier.get(&id, &context).await?.is_some() {
        return Err(io::Error::other("transaction probe survived rollback").into());
    }

    println!(
        "rust transaction/service smoke ok target=dev rolled_back=true audits={} events={}",
        audit_count, event_count
    );

    Ok(())
}

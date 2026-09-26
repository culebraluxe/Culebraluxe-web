use db::{Database, DbTarget, ProjectDao, ProjectTxDao};
use domain::{CreateProjectRequest, ProjectStatus, UpdateProjectRequest, WbsCategory};
use reqwest::Method;
use serde_json::{json, Value};
use server::projects::ProjectService;
use service::{
    CapturingAuditPort, CapturingDomainEventPort, CommandRequest, DefaultAuthorizationPort,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use std::error::Error;
use std::io;
use std::process::ExitCode;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = dispatch_cli(&args).await;

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("culebraluxe rust cli failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn dispatch_cli(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str).unwrap_or_default() {
        "db-smoke" => db_smoke().await,
        "tx-smoke" => tx_smoke().await,
        "service" => service_cli(&args[1..]).await,
        _ => {
            print_usage();
            Err(io::Error::other("unknown or missing command").into())
        }
    }
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  cargo run -p cli -- db-smoke");
    eprintln!("  cargo run -p cli -- tx-smoke");
    eprintln!("  cargo run -p cli -- service serve");
    eprintln!("  cargo run -p cli -- service catalog");
    eprintln!("  cargo run -p cli -- service health");
    eprintln!("  cargo run -p cli -- service status <domain>");
    eprintln!("  cargo run -p cli -- service drain <domain>");
    eprintln!("  cargo run -p cli -- service stop <domain>");
    eprintln!("  cargo run -p cli -- service dispatch <domain> <operation> [json-payload]");
    eprintln!("  cargo run -p cli -- service command '<CommandRequest json>'");
    eprintln!();
    eprintln!(
        "remote service commands require CULEBRA_CLI_AUTH_PROVIDER and CULEBRA_CLI_AUTH_SUB."
    );
}

async fn service_cli(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str).unwrap_or_default() {
        "serve" => server::http_runtime::run_http_server().await,
        "catalog" => print_remote_json(Method::GET, "/v1/services", None).await,
        "health" => print_remote_json(Method::GET, "/v1/services/runtime/health", None).await,
        "status" | "drain" | "stop" => {
            let command = args[0].as_str();
            let domain = args
                .get(1)
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| io::Error::other(format!("service {command} requires <domain>")))?;
            let path = format!("/v1/services/{domain}/control");
            print_remote_json(Method::POST, &path, Some(json!({ "command": command }))).await
        }
        "dispatch" => {
            let domain = args
                .get(1)
                .map(String::as_str)
                .ok_or_else(|| io::Error::other("service dispatch requires <domain>"))?;
            let operation = args
                .get(2)
                .map(String::as_str)
                .ok_or_else(|| io::Error::other("service dispatch requires <operation>"))?;
            let payload = match args.get(3) {
                Some(raw) => serde_json::from_str::<Value>(raw)
                    .map_err(|error| io::Error::other(format!("invalid JSON payload: {error}")))?,
                None => json!({}),
            };
            print_remote_json(
                Method::POST,
                "/v1/services/dispatch",
                Some(json!({
                    "domain": domain,
                    "operation": operation,
                    "payload": payload,
                })),
            )
            .await
        }
        "command" => {
            let raw = args
                .get(1)
                .ok_or_else(|| io::Error::other("service command requires CommandRequest JSON"))?;
            let command: CommandRequest = serde_json::from_str(raw).map_err(|error| {
                io::Error::other(format!("invalid CommandRequest JSON: {error}"))
            })?;
            let value = serde_json::to_value(command)?;
            print_remote_json(Method::POST, "/v1/commands/dispatch", Some(value)).await
        }
        _ => {
            print_usage();
            Err(io::Error::other("unknown service command").into())
        }
    }
}

struct RemoteServiceClient {
    client: reqwest::Client,
    base_url: String,
    internal_key: String,
    provider: String,
    subject: String,
}

impl RemoteServiceClient {
    fn from_env() -> Result<Self, Box<dyn Error>> {
        let base_url = std::env::var("CULEBRA_SERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".into())
            .trim_end_matches('/')
            .to_owned();
        let provider = required_env("CULEBRA_CLI_AUTH_PROVIDER")?;
        let subject = required_env("CULEBRA_CLI_AUTH_SUB")?;
        let config = server::api::ApiConfig::from_env().map_err(io::Error::other)?;
        Ok(Self {
            client: reqwest::Client::new(),
            base_url,
            internal_key: config.internal_api_key.to_string(),
            provider,
            subject,
        })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, Box<dyn Error>> {
        let correlation_id = format!("cli-{}", Uuid::new_v4());
        let mut request = self
            .client
            .request(method, format!("{}{}", self.base_url, path))
            .header("x-culebra-internal-key", &self.internal_key)
            .header("x-culebra-auth-provider", &self.provider)
            .header("x-culebra-auth-sub", &self.subject)
            .header("x-culebra-correlation-id", &correlation_id);
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        let value = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| {
            json!({
                "ok": false,
                "status": status.as_u16(),
                "message": text,
                "correlationId": correlation_id,
            })
        });
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "service API returned HTTP {}: {}",
                status,
                serde_json::to_string(&value)?
            ))
            .into());
        }
        Ok(value)
    }
}

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other(format!("{name} is required")).into())
}

async fn print_remote_json(
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<(), Box<dyn Error>> {
    let client = RemoteServiceClient::from_env()?;
    let value = client.request(method, path, body).await?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
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
            account_type: "internal".into(),
            entitlement_codes: vec![],
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

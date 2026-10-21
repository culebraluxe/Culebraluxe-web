mod apple_calls;
mod apple_contacts;
mod apple_mail;
mod apple_messages;
mod apple_sync;
mod db_tool;
mod gmail;
mod repo_root;
mod smoke;
mod ui_tool;

use db::{Database, DbTarget, ProjectDao, ProjectTxDao};
use model::{CreateProjectRequest, ProjectStatus, UpdateProjectRequest, WbsCategory};
use reqwest::Method;
use serde_json::{json, Value};
use services::{
    AbstractObjectStoreService, CapturingAuditPort, CapturingDomainEventPort, CommandRequest,
    DefaultAuthorizationPort, ObjectBucket, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::io;
use std::process::ExitCode;
use std::sync::Arc;
use uuid::Uuid;
use web::projects::ProjectService;

#[tokio::main]
async fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    // The database gates speak in exit codes (0 ok, 1 refused or drift, 2 configuration), and CI plus the
    // DEV_OPS playbook read them, so they own their exit path instead of the generic mapping below.
    if args.first().map(String::as_str) == Some("db-tool") {
        // THE OPERATOR'S LONG WORK IS NOT A REQUEST. Applying a migration that builds an index, or running a bulk
        // load, may legitimately take minutes; the per-statement ceiling the pool sets by default exists to stop a
        // stuck *request* from holding a connection, and cancelling a migration halfway is worse than waiting. So
        // `db-tool` — which is precisely the "apply this and wait" path — turns it off for its own process.
        db::disable_statement_timeout();
        return match db_tool::dispatch(&args[1..]).await {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("db-tool: {error}");
                ExitCode::from(error.exit_code())
            }
        };
    }

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
        "apple-sync" => apple_sync::dispatch(&args[1..]).await,
        // Google mail: metadata-only latest-context. Not an Apple source, so it is its own command.
        "gmail-sync" => gmail::gmail_sync(&args[1..]).await,
        // Google mail: the identity half of the same source — the approved census artifact into
        // relationship evidence. `gmail-sync` reads the identities this load exact-links.
        "gmail-census" => gmail::gmail_census(&args[1..]).await,
        // The post-deploy smoke, ported from `scripts/prod-smoke.ts` on 2026-09-28 (the last live TypeScript in
        // the release path). It exits non-zero when a check fails, which is how `pnpm release` notices.
        "smoke" => smoke::dispatch(&args[1..]).await,
        "ui" => ui_tool::dispatch(&args[1..]).await,
        "media-cards" => media_cards(&args[1..]).await,
        "media-variants" => media_variants(&args[1..]).await,
        _ => {
            print_usage();
            Err(io::Error::other("unknown or missing command").into())
        }
    }
}

/// Gives every stored photograph its CARD copy (1200px) — made for new uploads since 2026-09-28; this backfills the
/// rest. Safe to run again: it only touches photographs without one. One photograph at a time; a failure is reported
/// and skipped, never fatal to the others.
///
/// OBJECT-FIRST. The original is read from object storage (`original_object_location` + `get_object`, checked against
/// its HEAD size and SHA-256), the derived card is uploaded and verified, and only its object metadata is recorded
/// (`insert_copy_object`). This command no longer reads `media.file_data` and never writes a blob: without media
/// object storage it has nothing to do, so a missing store is a configuration error rather than a silent fallback.
async fn media_cards(args: &[String]) -> Result<(), Box<dyn Error>> {
    apple_sync::load_env();
    let target = match args.first().map(String::as_str) {
        Some("dev") => db::DbTarget::Dev,
        Some("prod") => db::DbTarget::Prod,
        _ => return Err(io::Error::other("usage: media-cards dev|prod").into()),
    };
    let database = db::Database::connect_target(target).await?;

    // No legacy fallback: the whole point of this command after the cutover is to write object rows. A deployment
    // without the store cannot create a card copy that the object-first read path can serve.
    let store: Arc<dyn AbstractObjectStoreService> = web::media::object_store_from_env()
        .map_err(|error| {
            io::Error::other(format!(
                "media-cards: media object storage is misconfigured: {error}"
            ))
        })?
        .map(|store| Arc::new(store) as Arc<dyn AbstractObjectStoreService>)
        .ok_or_else(|| {
            io::Error::other(
                "media-cards: media object storage is not configured; set MEDIA_OBJECT_STORE_* \
                 (this command reads originals from objects and never writes media.file_data)",
            )
        })?;

    let media = db::MediaDao::new(database);
    let (mut made, mut failed, mut saved_bytes) = (0usize, 0usize, 0i64);
    loop {
        let batch = media.images_missing_copy("card", 20).await?;
        if batch.is_empty() {
            break;
        }
        let mut progressed = false;
        for (id, filename) in batch {
            let label = filename.as_deref().unwrap_or(&id).to_owned();
            // The original's own record copy — never the servable copy — is what a derived card must be made from.
            let Some(location) = media.original_object_location(&id).await? else {
                failed += 1;
                eprintln!(
                    "skipped {label} — original has no object location; backfill it before card copies"
                );
                continue;
            };
            let Some(bucket) = ObjectBucket::from_db(&location.bucket) else {
                failed += 1;
                eprintln!(
                    "skipped {label} — unknown object bucket {}",
                    location.bucket
                );
                continue;
            };
            if location.status != "VERIFIED" {
                failed += 1;
                eprintln!(
                    "skipped {label} — original object is not VERIFIED ({})",
                    location.status
                );
                continue;
            }
            let original = match store.get_object(bucket, &location.key).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    failed += 1;
                    eprintln!("skipped {label} — original object could not be read: {error}");
                    continue;
                }
            };
            if let Some(problem) =
                verify_original_object(&*store, bucket, &location.key, &original).await
            {
                failed += 1;
                eprintln!("skipped {label} — {problem}");
                continue;
            }
            match web::media::imaging::derive_card(&original) {
                Ok(card) => {
                    // Deterministic per original: a failed DB insert can be retried over the same object without
                    // orphaning a second copy, and the row's gate (`images_missing_copy`) prevents a duplicate row.
                    let key = format!("media/{id}/card.jpg");
                    let stored = match store
                        .put_object(bucket, &key, &card.bytes, "image/jpeg")
                        .await
                    {
                        Ok(stored) => stored,
                        Err(error) => {
                            failed += 1;
                            eprintln!("skipped {label} — card object upload failed: {error}");
                            continue;
                        }
                    };
                    let Some(sha256) = stored.sha256.clone() else {
                        let _ = store.delete_object(bucket, &key).await;
                        failed += 1;
                        eprintln!(
                            "skipped {label} — provider returned no SHA-256 for the card object"
                        );
                        continue;
                    };
                    if let Err(error) = store
                        .verify_object(bucket, &key, &sha256, stored.size)
                        .await
                    {
                        let _ = store.delete_object(bucket, &key).await;
                        failed += 1;
                        eprintln!("skipped {label} — card object failed verification: {error}");
                        continue;
                    }
                    let object = db::MediaObjectMetadata {
                        bucket: bucket.as_str().to_owned(),
                        key: key.clone(),
                        etag: stored.etag,
                        sha256: Some(sha256),
                        size: stored.size,
                        status: "VERIFIED".to_owned(),
                        visibility: Some(location.visibility.clone()),
                    };
                    match media
                        .insert_copy_object(&id, "card", filename.as_deref(), &object)
                        .await
                    {
                        Ok(_) => {
                            made += 1;
                            saved_bytes += card.bytes.len() as i64;
                            progressed = true;
                            println!(
                                "card {made}: {label} ({}x{}, {} KB)",
                                card.width,
                                card.height,
                                card.bytes.len() / 1024
                            );
                        }
                        Err(error) => {
                            // No row points at this object now; remove it so the retry does not leave an orphan.
                            let _ = store.delete_object(bucket, &key).await;
                            return Err(error.into());
                        }
                    }
                }
                Err(reason) => {
                    failed += 1;
                    eprintln!("skipped {label} — {reason}");
                }
            }
        }
        // A batch where every photograph failed would come back forever; stop rather than spin.
        if !progressed {
            break;
        }
    }
    println!(
        "done: {made} card copies ({} MB), {failed} skipped",
        saved_bytes / 1_048_576
    );
    Ok(())
}

/// Gives every stored photograph the servable copies it never got: `web` (the full-size copy, capped under the
/// gateway's response limit) and `thumb` (the grid). One photograph at a time; safe to run again.
///
/// OBJECT-FIRST, like `media-cards`. The ORIGINAL is read from its object pointer and verified against the store's
/// HEAD (size + SHA-256), derived once with the same `imaging` module the upload path uses, uploaded under a
/// deterministic key (`media/{original_id}/{kind}.jpg`), verified, and recorded as a metadata-only `media` row
/// (`insert_copy_object`). No `media.file_data` is written; without media object storage there is nothing to do.
///
/// IDEMPOTENT: the work list is `images_missing_copy(kind)`, the object key is deterministic (a retry overwrites the
/// same object), and a row is only inserted for a kind the database says is missing. A failed DB insert deletes the
/// object it just wrote, so a retry does not leave an orphan. Per-photograph failures are reported and skipped; a batch
/// where nothing progressed stops the run rather than spinning.
async fn media_variants(args: &[String]) -> Result<(), Box<dyn Error>> {
    apple_sync::load_env();
    let target = match args.first().map(String::as_str) {
        Some("dev") => db::DbTarget::Dev,
        Some("prod") => db::DbTarget::Prod,
        _ => return Err(io::Error::other("usage: media-variants dev|prod [web|thumb]").into()),
    };
    let kinds = variant_kinds(&args[1..])?;
    let database = db::Database::connect_target(target).await?;

    let store: Arc<dyn AbstractObjectStoreService> = web::media::object_store_from_env()
        .map_err(|error| {
            io::Error::other(format!(
                "media-variants: media object storage is misconfigured: {error}"
            ))
        })?
        .map(|store| Arc::new(store) as Arc<dyn AbstractObjectStoreService>)
        .ok_or_else(|| {
            io::Error::other(
                "media-variants: media object storage is not configured; set MEDIA_OBJECT_STORE_* \
                 (this command reads originals from objects and never writes media.file_data)",
            )
        })?;

    let media = db::MediaDao::new(database);
    let mut made: std::collections::HashMap<&'static str, usize> =
        kinds.iter().map(|kind| (*kind, 0usize)).collect();
    let mut failed = 0usize;
    let mut saved_bytes = 0i64;

    loop {
        // One work list: every ORIGINAL missing at least one requested kind, with the kinds it still needs. Reading
        // both lists first is what lets a photograph missing web AND thumb be derived once, not twice.
        let mut needed: std::collections::BTreeMap<String, (Option<String>, Vec<&'static str>)> =
            std::collections::BTreeMap::new();
        for kind in &kinds {
            for (id, filename) in media.images_missing_copy(kind, 20).await? {
                needed
                    .entry(id)
                    .or_insert_with(|| (filename, Vec::new()))
                    .1
                    .push(*kind);
            }
        }
        if needed.is_empty() {
            break;
        }

        let mut progressed = false;
        for (id, (filename, missing_kinds)) in needed {
            let label = filename.clone().unwrap_or_else(|| id.clone());
            // The original's own record copy — never the servable copy — is what the derivatives are made from.
            let Some(location) = media.original_object_location(&id).await? else {
                failed += 1;
                eprintln!(
                    "skipped {label} — original has no object location; migrate it before variants"
                );
                continue;
            };
            let Some(bucket) = ObjectBucket::from_db(&location.bucket) else {
                failed += 1;
                eprintln!(
                    "skipped {label} — unknown object bucket {}",
                    location.bucket
                );
                continue;
            };
            if location.status != "VERIFIED" {
                failed += 1;
                eprintln!(
                    "skipped {label} — original object is not VERIFIED ({})",
                    location.status
                );
                continue;
            }
            let original = match store.get_object(bucket, &location.key).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    failed += 1;
                    eprintln!("skipped {label} — original object could not be read: {error}");
                    continue;
                }
            };
            if let Some(problem) =
                verify_original_object(&*store, bucket, &location.key, &original).await
            {
                failed += 1;
                eprintln!("skipped {label} — {problem}");
                continue;
            }
            // Decode once, derive once; pick the kinds this original is still missing.
            let derivatives = match web::media::imaging::derive_web_and_thumb(&original) {
                Ok(derivatives) => derivatives,
                Err(reason) => {
                    failed += 1;
                    eprintln!("skipped {label} — {reason}");
                    continue;
                }
            };
            for kind in missing_kinds {
                let Some(derivative) = derivatives
                    .iter()
                    .find(|derivative| derivative.kind == kind)
                else {
                    failed += 1;
                    eprintln!("skipped {label} — imaging produced no {kind} copy");
                    continue;
                };
                // Deterministic per original+kind: a retried upload overwrites the same object, so a failed row
                // insert cannot leave a second copy behind.
                let key = format!("media/{id}/{kind}.jpg");
                let stored = match store
                    .put_object(bucket, &key, &derivative.bytes, "image/jpeg")
                    .await
                {
                    Ok(stored) => stored,
                    Err(error) => {
                        failed += 1;
                        eprintln!("skipped {label} — {kind} object upload failed: {error}");
                        continue;
                    }
                };
                let Some(sha256) = stored.sha256.clone() else {
                    let _ = store.delete_object(bucket, &key).await;
                    failed += 1;
                    eprintln!(
                        "skipped {label} — provider returned no SHA-256 for the {kind} object"
                    );
                    continue;
                };
                if let Err(error) = store
                    .verify_object(bucket, &key, &sha256, stored.size)
                    .await
                {
                    let _ = store.delete_object(bucket, &key).await;
                    failed += 1;
                    eprintln!("skipped {label} — {kind} object failed verification: {error}");
                    continue;
                }
                let object = db::MediaObjectMetadata {
                    bucket: bucket.as_str().to_owned(),
                    key: key.clone(),
                    etag: stored.etag,
                    sha256: Some(sha256),
                    size: stored.size,
                    status: "VERIFIED".to_owned(),
                    visibility: Some(location.visibility.clone()),
                };
                match media
                    .insert_copy_object(&id, kind, filename.as_deref(), &object)
                    .await
                {
                    Ok(_) => {
                        *made.get_mut(&kind).expect("requested kind") += 1;
                        saved_bytes += derivative.bytes.len() as i64;
                        progressed = true;
                        println!(
                            "{kind} {}: {label} ({}x{}, {} KB)",
                            made[&kind],
                            derivative.width,
                            derivative.height,
                            derivative.bytes.len() / 1024
                        );
                    }
                    Err(error) => {
                        // No row points at this object now; remove it so the retry does not leave an orphan.
                        let _ = store.delete_object(bucket, &key).await;
                        return Err(error.into());
                    }
                }
            }
        }
        // A batch where every photograph failed would come back forever; stop rather than spin.
        if !progressed {
            break;
        }
    }

    let summary = kinds
        .iter()
        .map(|kind| format!("{} {kind} copies", made[kind]))
        .collect::<Vec<_>>()
        .join(" + ");
    println!(
        "done: {summary} ({} MB), {failed} skipped",
        saved_bytes / 1_048_576
    );
    Ok(())
}

/// The derivative kinds `media-variants` should backfill: the optional argument names one, omission means both.
fn variant_kinds(args: &[String]) -> Result<Vec<&'static str>, io::Error> {
    match args {
        [] => Ok(vec!["web", "thumb"]),
        [one] if one == "web" => Ok(vec!["web"]),
        [one] if one == "thumb" => Ok(vec!["thumb"]),
        _ => Err(io::Error::other("usage: media-variants dev|prod [web|thumb]").into()),
    }
}

/// Prove the downloaded original is the object the row points at before deriving anything from it.
///
/// The store's HEAD metadata is the reference when it reports a size/SHA-256; a provider that reports neither still
/// gets the read, but a reported mismatch is a refusal (a card made from swapped bytes would be served as the
/// original's).
async fn verify_original_object(
    store: &dyn AbstractObjectStoreService,
    bucket: ObjectBucket,
    key: &str,
    bytes: &[u8],
) -> Option<String> {
    let head = match store.head_object(bucket, key).await {
        Ok(head) => head,
        Err(error) => return Some(format!("original object HEAD failed: {error}")),
    };
    if head.size > 0 && head.size != bytes.len() as i64 {
        return Some(format!(
            "original object is {} bytes, expected {}",
            bytes.len(),
            head.size
        ));
    }
    if let Some(expected) = head.sha256.as_deref() {
        let actual = sha256_hex(bytes);
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Some("original object SHA-256 does not match its record".to_owned());
        }
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  cargo run -p cli -- db-smoke");
    eprintln!("  cargo run -p cli -- tx-smoke");
    eprintln!("  cargo run -p cli -- db-tool status");
    eprintln!("  cargo run -p cli -- db-tool apply <sql-file> [prod|dev] [--force] [--note \"…\"]");
    eprintln!("  cargo run -p cli -- db-tool parity");
    eprintln!("  cargo run -p cli -- apple-sync drain");
    eprintln!("  cargo run -p cli -- apple-sync calendar-intake <snapshot.json>");
    eprintln!("  cargo run -p cli -- apple-sync reminder-intake <snapshot.json>");
    eprintln!("  cargo run -p cli -- apple-sync messages-intake <export-dir> [--evidence-only] [--refresh]");
    eprintln!("  cargo run -p cli -- gmail-sync [dev|prod] [--window=N] [--verify]");
    eprintln!(
        "  cargo run -p cli -- gmail-census [dev|prod] [--file <census.csv>] [--verify] [--refresh]"
    );
    eprintln!("  cargo run -p cli -- ui capture-fixtures [base-url]");
    eprintln!("  cargo run -p cli -- media-cards <dev|prod>");
    eprintln!("  cargo run -p cli -- media-variants <dev|prod> [web|thumb]");
    eprintln!();
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
        "serve" => web::http_runtime::run_http_server().await,
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
        let config = web::api::ApiConfig::from_env().map_err(io::Error::other)?;
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

#[cfg(test)]
mod media_variants_tests {
    use super::variant_kinds;

    #[test]
    fn the_kind_argument_names_one_copy_or_both() {
        assert_eq!(variant_kinds(&[]).unwrap(), vec!["web", "thumb"]);
        assert_eq!(variant_kinds(&["web".to_owned()]).unwrap(), vec!["web"]);
        assert_eq!(variant_kinds(&["thumb".to_owned()]).unwrap(), vec!["thumb"]);
        assert!(variant_kinds(&["card".to_owned()]).is_err());
        assert!(variant_kinds(&["web".to_owned(), "thumb".to_owned()]).is_err());
    }
}

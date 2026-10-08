//! Operator tool for the control-plane databases: migration ledger status, migration apply, schema parity, and
//! carrying saved forms forward to a newer template version.
//!
//! Rust replacement for the retired TypeScript scripts behind `pnpm db:migrations`, `pnpm db:migrate` and
//! `pnpm db:parity` (`scripts/migration-status.mjs`, `scripts/apply-migration.mjs`,
//! `scripts/check-schema-parity.ts`). Those imported `legacy/db/forge-db.ts`, deleted with the TypeScript
//! application in `4cf98110`, so every database gate exited `ERR_MODULE_NOT_FOUND`. Same job, same script
//! names, one language: there is no Node runtime left in this repository to run them with.
//!
//! Usage:
//!   cargo run -p cli -- db-tool status
//!   cargo run -p cli -- db-tool apply <sql-file> [prod|dev] [--force] [--note "…"]
//!   cargo run -p cli -- db-tool parity
//!   cargo run -p cli -- db-tool carry-forward <template-id> <to-version> [prod|dev] [--from N] [--apply]
//!
//! Exit codes are part of the contract, because CI and the SOP read them:
//!   0 success (including "already applied, checksum matches"), 1 drift or refused, 2 configuration/usage.
//!
//! Guards carried over intact from `apply-migration.mjs`, because they are the reason the ledger is
//! trustworthy: a file already recorded with the SAME checksum is skipped, and a file recorded with a
//! DIFFERENT checksum is REFUSED unless `--force` is passed. Nothing here decides schema truth — it
//! executes a reviewed SQL file and records that it ran.

use db::{schema_parity, Database, DbTarget, FormDao, MigrationLedgerRow, SchemaMigrationDao};
use model::forms_carry_forward::{carried_sections, carried_values, CarriedBody};
use model::forms_template::TemplateLibrary;
use model::{
    CreateFormInstanceRequest, FormInstanceStatus, UpdateFormInstanceInput,
    UpdateFormInstanceRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// Where migration files live. The TypeScript scripts read this same directory, and the ledger records
/// filenames WITH this prefix, so it is the join key between disk and ledger — not a preference.
const MIGRATIONS_DIR: &str = "db/migrations";

/// The ledger migration itself, named in the "ledger is missing" message.
const LEDGER_MIGRATION: &str = "db/migrations/144_schema_migration_ledger.sql";

/// The synthetic row that documents the verified parity baseline; it is not a file.
const BASELINE: &str = "<baseline>";

pub async fn dispatch(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD; the same loader the rest of the CLI uses.
    crate::apple_sync::load_env();
    match args.first().map(String::as_str).unwrap_or_default() {
        "status" => status().await,
        "apply" => apply(&args[1..]).await,
        "parity" => parity().await,
        "carry-forward" => carry_forward(&args[1..]).await,
        _ => {
            usage();
            Ok(2)
        }
    }
}

pub fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo run -p cli -- db-tool status");
    eprintln!("  cargo run -p cli -- db-tool apply <sql-file> [prod|dev] [--force] [--note \"…\"]");
    eprintln!("  cargo run -p cli -- db-tool parity");
    eprintln!(
        "  cargo run -p cli -- db-tool carry-forward <template-id> <to-version> [prod|dev] [--from N] [--apply]"
    );
}

/// A failure that carries the exit code the scripts' contract gives it.
///
/// The retired scripts distinguished two kinds of failure, and CI still reads the difference: 0 applied or
/// clean, 1 refused or drift, 2 "cannot even start" — a missing connection URL, an absent ledger, bad
/// arguments. `check-schema-parity.ts` exited 2 exactly this way when `DATABASE_URL_DEV` /
/// `DATABASE_URL_PROD` were unset, and the distinction is worth keeping: a clean run and an unstartable run
/// must never look alike.
#[derive(Debug)]
pub enum Failure {
    /// Bad arguments: nothing was attempted.
    Usage(String),
    /// The environment or the database cannot support the run at all.
    Configuration(String),
    /// The work ran and failed: a refused apply, drift, a database error.
    Other(String),
}

impl Failure {
    pub fn exit_code(&self) -> u8 {
        match self {
            Failure::Usage(_) | Failure::Configuration(_) => 2,
            Failure::Other(_) => 1,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Usage(message) | Failure::Configuration(message) | Failure::Other(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for Failure {}

impl From<db::DbFailure> for Failure {
    fn from(failure: db::DbFailure) -> Self {
        // Configuration is checked before any connection is attempted (`require_env`), so anything that
        // reaches here is work that started and failed. `DbFailure`'s own text already carries the
        // driver's message and the sqlstate, which is the half an operator can act on.
        Failure::Other(failure.to_string())
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Failure::Other(error.to_string())
    }
}

/// Fail before touching a database when a target's URL is not configured.
///
/// Mirrors the scripts' own precondition ("DATABASE_URL_DEV and DATABASE_URL_PROD are required", exit 2)
/// rather than letting the pool surface it, so an unconfigured environment is never reported as drift.
/// `connect_target` still fails closed on its own; this only decides the exit code.
fn require_env(targets: &[DbTarget]) -> Result<(), Failure> {
    let missing: Vec<&str> = targets
        .iter()
        .map(|target| target_env_name(*target))
        .filter(|name| std::env::var(name).is_err())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(Failure::Configuration(format!(
        "{} {} required (set them in .env.local or the process environment)",
        missing.join(" and "),
        if missing.len() == 1 { "is" } else { "are" }
    )))
}

/// Connect to one named target, and say which database that actually is before doing any work.
///
/// `Database::connect_target` fails closed when the target's URL is not configured — it never falls back
/// to the other environment. The banner repeats that out loud, so no transcript of a gate run is
/// ambiguous about where it pointed.
async fn connect(target: DbTarget) -> Result<Database, Failure> {
    let database = Database::connect_target(target).await?;
    println!(
        "database: target={} {}",
        target.as_str(),
        host_label(target)
    );
    Ok(database)
}

fn target_env_name(target: DbTarget) -> &'static str {
    match target {
        DbTarget::Dev => "DATABASE_URL_DEV",
        DbTarget::Prod => "DATABASE_URL_PROD",
    }
}

fn host_label(target: DbTarget) -> String {
    match std::env::var(target_env_name(target)) {
        Ok(url) => format!("host={}", host_of(&url)),
        Err(_) => format!("host=({} is unset)", target_env_name(target)),
    }
}

/// Host only. A connection URL carries credentials, and a gate's output gets pasted into reports.
fn host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let after_userinfo = after_scheme.rsplit('@').next().unwrap_or(after_scheme);
    after_userinfo
        .split(['/', '?'])
        .next()
        .unwrap_or_default()
        .to_string()
}

fn migrations_dir() -> PathBuf {
    crate::apple_sync::repo_root().join(MIGRATIONS_DIR)
}

/// Basenames of the migration files, sorted, exactly as the TypeScript status script enumerated them.
fn migration_files(dir: &Path) -> Result<Vec<String>, Failure> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| io::Error::other(format!("cannot read {}: {error}", dir.display())))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "sql") {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                files.push(name.to_string());
            }
        }
    }
    files.sort();
    Ok(files)
}

/// The ledger's key for a file: its NAME. The ledger stores the path a file had when it was applied, and the folder
/// has moved (db/ -> legacy/db/ -> db/), so rows are matched by file name, whatever folder they were recorded under.
fn ledger_key(file: &str) -> String {
    file.rsplit('/').next().unwrap_or(file).to_owned()
}

/// `db:migrations` — what is recorded as applied, where, and what is not.
///
/// "Unrecorded" is honest, not alarming: the ledger is authoritative only from the 2026-09-10 baseline
/// forward, so files that predate it show up there by design.
async fn status() -> Result<u8, Failure> {
    require_env(&[DbTarget::Dev, DbTarget::Prod])?;
    let dev = SchemaMigrationDao::new(connect(DbTarget::Dev).await?);
    let prod = SchemaMigrationDao::new(connect(DbTarget::Prod).await?);

    for (target, ledger) in [(DbTarget::Dev, &dev), (DbTarget::Prod, &prod)] {
        if !ledger.present().await? {
            eprintln!(
                "schema_migration ledger is missing on {} — apply {LEDGER_MIGRATION} first",
                target.as_str()
            );
            return Ok(2);
        }
    }

    let files = migration_files(&migrations_dir())?;
    let dev_rows = dev.rows(DbTarget::Dev).await?;
    let prod_rows = prod.rows(DbTarget::Prod).await?;

    let dev_recorded = recorded_by_filename(&dev_rows);
    let prod_recorded = recorded_by_filename(&prod_rows);

    println!(
        "ledger: {} rows   migrations on disk: {}",
        dev_rows.len() + prod_rows.len(),
        files.len()
    );
    println!("  dev  recorded: {}", dev_rows.len());
    println!("  prod recorded: {}", prod_rows.len());

    let mut ledger: Vec<&MigrationLedgerRow> = dev_rows.iter().chain(prod_rows.iter()).collect();
    // Newest first: each target arrives ordered, this orders the two together.
    ledger.sort_by(|left, right| right.applied_at.cmp(&left.applied_at));

    println!("\nrecently recorded:");
    for row in ledger
        .iter()
        .filter(|row| row.filename != BASELINE)
        .take(12)
    {
        let note = match row.note.as_deref() {
            Some(note) if !note.is_empty() => format!("  — {note}"),
            _ => String::new(),
        };
        println!(
            "  {}  {:<4}  {}{}",
            row.applied_on, row.target, row.filename, note
        );
    }

    let baseline: Vec<&&MigrationLedgerRow> = ledger
        .iter()
        .filter(|row| row.filename == BASELINE)
        .collect();
    if !baseline.is_empty() {
        println!("\nbaseline rows:");
        for row in baseline {
            println!(
                "  {}  {:<4}  {}",
                row.applied_on,
                row.target,
                row.note.as_deref().unwrap_or_default()
            );
        }
    }

    let unrecorded: Vec<&String> = files
        .iter()
        .filter(|file| {
            let key = ledger_key(file);
            !dev_recorded.contains_key(&key) && !prod_recorded.contains_key(&key)
        })
        .collect();
    println!(
        "\nunrecorded (pre-baseline or never applied here): {}",
        unrecorded.len()
    );
    for file in unrecorded.iter().take(15) {
        println!("  {file}");
    }
    if unrecorded.len() > 15 {
        println!("  … and {} more", unrecorded.len() - 15);
    }

    let one_sided: Vec<(&String, bool)> = files
        .iter()
        .filter_map(|file| {
            let key = ledger_key(file);
            let on_dev = dev_recorded.contains_key(&key);
            let on_prod = prod_recorded.contains_key(&key);
            if (on_dev || on_prod) && !(on_dev && on_prod) {
                Some((file, on_dev))
            } else {
                None
            }
        })
        .collect();
    if !one_sided.is_empty() {
        println!(
            "\nrecorded for ONE target only (check whether that is intended): {}",
            one_sided.len()
        );
        for (file, on_dev) in one_sided {
            let side = if on_dev { "dev" } else { "prod" };
            println!("  {file}  [{side} only]");
        }
    }

    // Honesty line. Rows recorded under another folder are still matched (by file name); the report says how many.
    let mut elsewhere: BTreeMap<String, usize> = BTreeMap::new();
    for row in ledger.iter() {
        if let Some((directory, _)) = row.filename.rsplit_once('/') {
            if directory != MIGRATIONS_DIR {
                *elsewhere.entry(directory.to_string()).or_default() += 1;
            }
        }
    }
    if !elsewhere.is_empty() {
        let prefixes = elsewhere
            .iter()
            .map(|(directory, count)| format!("{directory} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "\nnote: the ledger also holds rows recorded under {prefixes}; this run scanned {MIGRATIONS_DIR} and \
             matched them by file name."
        );
    }

    Ok(0)
}

fn recorded_by_filename(rows: &[MigrationLedgerRow]) -> BTreeMap<String, &MigrationLedgerRow> {
    let mut map = BTreeMap::new();
    for row in rows {
        map.insert(ledger_key(&row.filename), row);
    }
    map
}

/// `db:migrate` — apply a migration SQL file to one control plane and record it in `schema_migration`.
///
/// The file is executed as ONE simple query, so a multi-statement migration behaves as written (each file
/// carries its own `begin`/`commit`, which is why the ledger row is written separately afterwards rather
/// than inside a transaction this tool opened). The ledger records the file NAME, not the path as given:
/// readers match by basename (the folder has moved twice), so the writer normalizes too.
async fn apply(args: &[String]) -> Result<u8, Failure> {
    let mut file: Option<String> = None;
    let mut force = false;
    let mut note: Option<String> = None;
    let mut explicit: Option<DbTarget> = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--force" => force = true,
            "--note" => {
                note = args.get(index + 1).cloned();
                index += 1;
            }
            "dev" => explicit = Some(DbTarget::Dev),
            "prod" => explicit = Some(DbTarget::Prod),
            other if !other.starts_with("--") && file.is_none() => file = Some(other.to_string()),
            _ => {}
        }
        index += 1;
    }

    let Some(file) = file else {
        usage();
        return Err(Failure::Usage("apply requires a SQL file path".to_string()));
    };
    let target = match explicit {
        Some(target) => target,
        None => default_target(),
    };
    let which = target.as_str();
    require_env(&[target])?;

    let sql = std::fs::read_to_string(&file)
        .map_err(|error| io::Error::other(format!("cannot read {file}: {error}")))?;
    let checksum = format!("sha256:{}", hex_digest(&sql));
    let database = connect(target).await?;
    let ledger = SchemaMigrationDao::new(database.clone());

    if !ledger.present().await? {
        eprintln!("schema_migration ledger is missing on {which} — apply {LEDGER_MIGRATION} first");
        return Ok(2);
    }

    if let Some(recorded) = ledger.recorded_checksum(&file, target).await? {
        if !force {
            if recorded == checksum {
                println!("already applied {file} -> {which} (checksum match) — skipped");
                return Ok(0);
            }
            eprintln!("REFUSED: {file} is already recorded for {which} with a DIFFERENT checksum.");
            eprintln!("  recorded: {recorded}");
            eprintln!("  current : {checksum}");
            eprintln!("  the file changed after it was applied — review, then re-run with --force");
            return Ok(1);
        }
    }

    database.run_text(&sql).await?;
    ledger
        .record(&file, &checksum, target, note.as_deref())
        .await?;

    println!("applied {file} -> {which} control plane (recorded in schema_migration)");
    Ok(0)
}

/// `carry-forward` — put the forms saved on an older template version onto a newer one, as new forms.
///
/// WHY A NEW FORM AND NOT A RE-STAMP. An instance's `template_version` is stamped once, at creation, and it is the
/// version the composer renders that document from — so moving the stamp would make a row claim a version it was never
/// authored or issued under. Everything else in this repository already reads that way: the v5 template's own header
/// says "Forms already issued on v4 keep v4; new forms are v5", an issued form is LOCKED in the editor
/// (`web/ui/src/app/screens/forms/editor.rs`), and the issued PDF with its snapshot sits in the vault under its own
/// template version. So this creates the newer form BESIDE the older one and touches neither the older row nor the
/// vault.
///
/// SAFE TO RUN TWICE. A form already on the target version for the same seller and property is FILLED instead of
/// duplicated — the older agreement's values win, the newer draft keeps what the older form has nothing for, and every
/// value the fill replaces is named in the report. `model::forms_carry_forward` owns that rule and states it. An
/// `issued` form on the target version is refused outright: an issued form is evidence, never a draft to edit.
///
/// DRY RUN BY DEFAULT, because this writes business rows: pass `--apply` to write them.
async fn carry_forward(args: &[String]) -> Result<u8, Failure> {
    let mut template_id: Option<String> = None;
    let mut to_version: Option<i32> = None;
    let mut from_version: Option<i32> = None;
    let mut apply = false;
    let mut explicit: Option<DbTarget> = None;

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--apply" => apply = true,
            "--from" => {
                from_version = args
                    .get(index + 1)
                    .and_then(|value| value.parse::<i32>().ok());
                index += 1;
            }
            "dev" => explicit = Some(DbTarget::Dev),
            "prod" => explicit = Some(DbTarget::Prod),
            other if !other.starts_with("--") && template_id.is_none() => {
                template_id = Some(other.to_string())
            }
            other if !other.starts_with("--") && to_version.is_none() => {
                let Ok(parsed) = other.parse::<i32>() else {
                    return Err(Failure::Usage(format!(
                        "{other} is not a template version — pass the version to carry forward TO, e.g. `carry-forward LISTING-01 5`"
                    )));
                };
                to_version = Some(parsed);
            }
            _ => {}
        }
        index += 1;
    }

    let (Some(template_id), Some(to_version)) = (template_id, to_version) else {
        usage();
        return Err(Failure::Usage(
            "carry-forward requires a template id and the version to carry forward TO, e.g. `carry-forward LISTING-01 5`"
                .to_string(),
        ));
    };

    let target = explicit.unwrap_or_else(default_target);
    require_env(&[target])?;

    // The templates are authoring FILES read at runtime, so "does v5 exist" is a question for the directory, never for
    // a constant — and a version that is not there is a usage error, not an empty run.
    let library = TemplateLibrary::load_default().map_err(|error| {
        Failure::Configuration(format!("the form templates did not load: {error}"))
    })?;
    let Some(newer) = library.version(&template_id, to_version) else {
        let known: Vec<String> = library
            .families()
            .iter()
            .filter(|(id, _)| *id == template_id)
            .map(|(_, version)| format!("v{version}"))
            .collect();
        return Err(Failure::Usage(format!(
            "{template_id} v{to_version} is not in the template directory (it has {})",
            join_or_none(&known)
        )));
    };
    let to_version = newer.version;

    let database = connect(target).await?;
    let forms = FormDao::new(database);
    let instances = forms.list_instances().await?;

    let mut sources: Vec<usize> = instances
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            let form = &item.instance;
            let from_matches = match from_version {
                Some(from) => form.template_version == from,
                None => true,
            };
            form.template_id == template_id && form.template_version < to_version && from_matches
        })
        .map(|(index, _)| index)
        .collect();
    // Oldest version first: the report reads in the order the versions were authored, not in the list's updated_at order.
    sources.sort_by(|left, right| {
        let left = &instances[*left].instance;
        let right = &instances[*right].instance;
        (left.template_version, left.updated_at.as_str())
            .cmp(&(right.template_version, right.updated_at.as_str()))
    });

    println!(
        "{template_id} v{to_version}: {} form(s) saved on an older version",
        sources.len()
    );
    if sources.is_empty() {
        return Ok(0);
    }
    if !apply {
        println!("dry run — nothing is written; pass --apply to carry them forward");
    }

    let empty: BTreeMap<String, String> = BTreeMap::new();
    let (mut creates, mut fills, mut skips) = (0, 0, 0);

    for index in sources {
        let item = &instances[index];
        let source = &item.instance;
        println!(
            "\n  {} v{} {}   {} / {}",
            source.template_id,
            source.template_version,
            short_id(&source.id),
            item.client_name.as_deref().unwrap_or("(no seller)"),
            item.property_label.as_deref().unwrap_or("(no property)")
        );

        // Without the older version on disk there is nothing to compare its prose against, and a copy of a document
        // nobody can render is not a conversion. Named, not guessed.
        let Some(older) = library.version(&source.template_id, source.template_version) else {
            println!(
                "    SKIPPED — v{} is not in the template directory, so what it prints cannot be compared",
                source.template_version
            );
            skips += 1;
            continue;
        };

        let existing = instances.iter().map(|item| &item.instance).find(|other| {
            other.template_id == template_id
                && other.template_version == to_version
                && other.person_id.is_some()
                && other.person_id == source.person_id
                && other.property_id.is_some()
                && other.property_id == source.property_id
        });
        let (existing_values, existing_sections) = match existing {
            Some(other) if other.status == FormInstanceStatus::Issued => {
                println!(
                    "    SKIPPED — v{to_version} {} is already issued for this seller and property; an issued form is evidence",
                    short_id(&other.id)
                );
                skips += 1;
                continue;
            }
            Some(other) => (&other.field_values, &other.sections),
            None => (&empty, &empty),
        };

        // The email the product itself resolves for this form's own signer — the same rule the signing flow uses
        // (`FormDao::list_signer_people`), so a filled address is the address a signing link would go to.
        let derived_email = forms
            .list_signer_people(&source.id)
            .await?
            .into_iter()
            .find(|person| person.person_id.as_deref() == source.person_id.as_deref())
            .and_then(|person| person.email)
            .map(|email| email.trim().to_string())
            .filter(|email| !email.is_empty());

        let carried = carried_values(
            older,
            newer,
            &source.field_values,
            existing_values,
            derived_email.as_deref(),
        );
        let (sections, body) = carried_sections(older, newer, &source.sections, existing_sections);

        let action = if existing.is_some() { "fill" } else { "create" };
        let email_note = match carried.derived_email.as_deref() {
            Some(field) => format!("{field} from the person's record"),
            None => "no field the older form could not know".to_string(),
        };
        let body_note = match body {
            CarriedBody::CarriedTheOlderProse => format!(
                "carries the v{} edited prose (v{} and v{to_version} print the same sections)",
                source.template_version, source.template_version
            ),
            CarriedBody::KeptTheNewerEdit => format!("keeps the v{to_version} form's own edit"),
            CarriedBody::Regenerated => format!(
                "composed from v{to_version} (no edit to carry, or v{} does not print what v{to_version} prints)",
                source.template_version
            ),
        };
        println!(
            "    would {action} v{to_version}: {} value(s) filled, {email_note}, body {body_note}",
            carried
                .values
                .values()
                .filter(|value| !value.trim().is_empty())
                .count()
        );
        if !carried.dropped.is_empty() {
            println!(
                "      not carried (v{to_version} declares no such field): {}",
                carried.dropped.join(", ")
            );
        }
        for (field, previous) in &carried.replaced {
            println!(
                "      replaces {field}: {previous} → {}",
                carried
                    .values
                    .get(field)
                    .map(String::as_str)
                    .unwrap_or_default()
            );
        }
        if source.contract_id.is_some() {
            println!(
                "      note: the contract lineage stays on the v{} form {}",
                source.template_version,
                short_id(&source.id)
            );
        }

        match existing {
            Some(other) => {
                fills += 1;
                if !apply {
                    continue;
                }
                let updated = forms
                    .update_instance(&UpdateFormInstanceRequest {
                        form_instance_id: other.id.clone(),
                        input: UpdateFormInstanceInput {
                            field_values: Some(carried.values),
                            sections: Some(sections),
                            status: None,
                            contract_id: None,
                        },
                    })
                    .await?;
                match updated {
                    Some(form) => println!("    filled v{} {}", form.template_version, form.id),
                    None => {
                        println!(
                            "    NOT FOUND: {} disappeared between the read and the write",
                            other.id
                        );
                        skips += 1;
                    }
                }
            }
            None => {
                creates += 1;
                if !apply {
                    continue;
                }
                // `contract_id` is deliberately NOT carried: it is the lineage of the form that was ISSUED, and this
                // new draft has issued nothing. The context check needs one of contract/deal/person/property, and the
                // seller and the property do travel.
                let created = forms
                    .create_instance(&CreateFormInstanceRequest {
                        template_id: template_id.clone(),
                        template_version: to_version,
                        deal_id: source.deal_id.clone(),
                        person_id: source.person_id.clone(),
                        property_id: source.property_id.clone(),
                        field_values: carried.values,
                        sections,
                        created_by_user_id: source.created_by_user_id.clone(),
                    })
                    .await?;
                println!(
                    "    created v{} {} (draft) for {} / {}",
                    created.template_version,
                    created.id,
                    item.client_name.as_deref().unwrap_or("(no seller)"),
                    item.property_label.as_deref().unwrap_or("(no property)")
                );
            }
        }
    }

    if apply {
        println!("\ncreated {creates}, filled {fills}, skipped {skips}");
    } else {
        println!(
            "\ndry run — nothing written: {creates} would be created, {fills} filled, {skips} skipped"
        );
    }
    Ok(0)
}

/// The first eight characters of a uuid: enough to name a row in a report, short enough to read.
fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// An explicit target argument wins. Otherwise the declared environment decides, as the script did
/// (`APP_ENV=production` -> prod); silence defaults to DEV, which is the free environment. The resolved
/// target is always printed by `connect` before anything executes, so a default is never a surprise.
fn default_target() -> DbTarget {
    db::resolve_declared_target(
        std::env::var("VERCEL_ENV").ok().as_deref(),
        std::env::var("APP_ENV").ok().as_deref(),
    )
    .unwrap_or(DbTarget::Dev)
}

fn hex_digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `db:parity` — the release gate. DEV vs PROD structural comparison, read-only, exits non-zero on ANY
/// drift so it can gate a release.
async fn parity() -> Result<u8, Failure> {
    require_env(&[DbTarget::Dev, DbTarget::Prod])?;
    let dev = connect(DbTarget::Dev).await?;
    let prod = connect(DbTarget::Prod).await?;

    let (dev_snapshot, prod_snapshot) = tokio::try_join!(
        schema_parity::read_snapshot(&dev),
        schema_parity::read_snapshot(&prod)
    )?;
    let report = schema_parity::compare_snapshots(&dev_snapshot, &prod_snapshot);

    println!(
        "tables only in DEV : {}",
        join_or_none(&report.tables_only_dev)
    );
    println!(
        "tables only in PROD: {}",
        join_or_none(&report.tables_only_prod)
    );
    for (label, lines) in [
        ("column drift", &report.column_drift),
        ("index drift ", &report.index_drift),
        ("fk drift    ", &report.fk_drift),
        ("check drift ", &report.check_drift),
    ] {
        println!("{label}: {}", lines.len());
        for line in lines {
            println!("  {line}");
        }
    }

    if report.clean {
        println!("\nPARITY OK");
        Ok(0)
    } else {
        println!("\nDRIFT FOUND");
        Ok(1)
    }
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{host_of, Failure};

    /// The exit codes are CI's contract: 0 applied or clean, 1 refused or drift, 2 unstartable.
    #[test]
    fn failure_exit_codes_match_the_scripts_contract() {
        assert_eq!(Failure::Usage("bad arguments".to_string()).exit_code(), 2);
        assert_eq!(
            Failure::Configuration("missing URL".to_string()).exit_code(),
            2
        );
        assert_eq!(Failure::Other("drift".to_string()).exit_code(), 1);
    }

    /// Gate output gets pasted into reports and transcripts, so the banner must never carry credentials.
    #[test]
    fn host_of_reports_only_the_host() {
        assert_eq!(
            host_of("postgresql://neondb_owner:npg_SECRET@ep-cool-db-12345.us-east-2.aws.neon.tech/neondb?sslmode=require"),
            "ep-cool-db-12345.us-east-2.aws.neon.tech"
        );
        assert_eq!(
            host_of("postgres://user:pw@localhost:5432/db"),
            "localhost:5432"
        );
    }
}

// ---------------------------------------------------------------------------
// Apple Contacts — operator NOTES merge.
//
// WHY THIS EXISTS: macOS gates `CNContact.note` behind the
// `com.apple.developer.contacts.notes` entitlement, which a plain local Swift build does not have,
// so the exporter can only ever emit "". The Contacts app ITSELF can read notes (it owns them), and
// AppleScript reaches it without any entitlement. One AppleScript pass asks the app for
// `id<TAB>base64(note)` for every person that HAS a note, and that note is written onto the matching
// export contact.
//
// Join key: AppleScript `id of person` IS the ABPerson identifier — the export's `sourceId` (and
// `l_person.source_contact_id`). Notes are CONTEXT (operator memory aids), never a name and never an
// identity, so a failure here must never block the load: the export is left exactly as it was.
//
// The AppleScript is BULK on purpose: a `repeat with p in people ... note of p` loop costs one IPC
// round trip PER PERSON (measured 336s over 4,662 people); fetching the whole property list once and
// iterating it in-script is ~5s for the same data.
// ---------------------------------------------------------------------------
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const APPLESCRIPT: &str = r#"
set out to ""
tell application "Contacts"
  set ids to id of people
  set ns to note of people
  set total to count of ids
  repeat with k from 1 to total
    set n to item k of ns
    if n is not missing value and (n as text) is not "" then
      set b64 to do shell script "printf %s " & quoted form of (n as text) & " | base64"
      set out to out & (item k of ids) & tab & b64 & linefeed
    end if
  end repeat
end tell
return out
"#;

/// `id<TAB>base64(note)` per person that has one. An undecodable note is skipped rather than losing
/// the whole run.
fn read_notes() -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(APPLESCRIPT)
        .output()
        .map_err(|error| io::Error::other(format!("osascript could not run: {error}")))?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "AppleScript failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut notes = BTreeMap::new();
    for line in stdout.lines() {
        let Some(tab) = line.find('\t') else {
            continue;
        };
        let id = line[..tab].trim();
        let encoded = line[tab + 1..].trim();
        if id.is_empty() || encoded.is_empty() {
            continue;
        }
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) {
            notes.insert(id.to_owned(), String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    Ok(notes)
}

/// Write the export back atomically: a half-written export is worse than a merge that did not run.
fn write_atomically(path: &Path, batch: &Value) -> Result<(), Box<dyn Error>> {
    let tmp = PathBuf::from(format!("{}.tmp-notes", path.display()));
    fs::write(&tmp, serde_json::to_string_pretty(batch)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// One person's note, when the source id names a contact and the note says something.
fn merge_note(contact: &mut Value, note: &str) -> bool {
    if note.trim().is_empty() {
        return false;
    }
    contact
        .as_object_mut()
        .map(|object| {
            object.insert("note".to_owned(), Value::String(note.to_owned()));
        })
        .is_some()
}

/// `apple-sync contacts-notes --file <contacts-export.json> [--quiet]`
pub async fn contacts_notes(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let Some(file) = crate::apple_mail::option(args, "--file").map(PathBuf::from) else {
        return Err(io::Error::other("--file <contacts-export.json> is required").into());
    };
    let quiet = args.iter().any(|arg| arg == "--quiet");
    let started = Instant::now();

    let notes = match read_notes() {
        Ok(notes) => notes,
        Err(error) => {
            // Never fail the load: notes are context, and the artifact is left untouched.
            eprintln!("[notes] {error}");
            eprintln!("[notes] contacts export left unchanged (notes absent for this run)");
            return Ok(());
        }
    };

    let raw = fs::read_to_string(&file).map_err(|error| {
        io::Error::other(format!(
            "contacts export unreadable at {}: {error}",
            file.display()
        ))
    })?;
    let mut batch: Value = serde_json::from_str(&raw).map_err(|error| {
        io::Error::other(format!("{} is not valid JSON: {error}", file.display()))
    })?;

    let Some(contacts) = batch.get_mut("contacts").and_then(Value::as_array_mut) else {
        return Err(io::Error::other(format!(
            "{} has no contacts array; refusing to rewrite it",
            file.display()
        ))
        .into());
    };
    let total = contacts.len();
    let mut merged = 0usize;
    for contact in contacts.iter_mut() {
        let Some(source_id) = contact.get("sourceId").and_then(Value::as_str) else {
            continue;
        };
        if let Some(note) = notes.get(source_id) {
            if merge_note(contact, note) {
                merged += 1;
            }
        }
    }

    write_atomically(&file, &batch)?;

    if !quiet {
        println!(
            "[notes] people with notes: {}; merged onto export contacts: {merged}/{total} ({:.1}s)",
            notes.len(),
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// `apple-sync warehouse-promote [dev|prod] [--apply]`
///
/// The landing -> warehouse step of the Contacts chain (`scripts/contacts-sync.sh`), which used to be
/// `scripts/promote-warehouse.ts` — read every landing row into Node, decide in memory, push back.
/// The rules are a database function now; this is the shell that calls it and prints its tally.
pub async fn warehouse_promote(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = crate::apple_mail::target_arg(args)?;
    let apply = args.iter().any(|arg| arg == "--apply");
    println!(
        "[promote] landing -> warehouse (target={}, apply={apply})",
        target.as_str()
    );

    let database = crate::apple_mail::connect(target).await?;
    let landing = db::LandingDao::new(database);
    let tally = landing.promote_apple_contacts(apply).await?;
    println!("{tally}");

    if apply {
        landing.refresh_client_read_models().await?;
        println!("[promote] client read models refreshed");
    }
    Ok(())
}

/// `apple-sync contacts-load --file <export.json> --source-account <account> [dev|prod]`
///
/// The ODS load step of the Contacts chain (`scripts/contacts-sync.sh`), which used to be
/// `scripts/load-apple-contacts.ts`: read the export, then pull every contact out to Node, normalize it
/// there and push it back. The rules are a database function now
/// (`db/migrations/254_apple_contacts_load_project.sql`); this reads the file, keeps the raw contacts
/// exactly as exported, adds the file's own sha256, and prints the tally the function returns.
///
/// A refusal (a batch id whose bytes changed) exits non-zero so the wrapper stops the chain.
pub async fn contacts_load(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = crate::apple_mail::target_arg(args)?;
    let Some(file) = crate::apple_mail::option(args, "--file")
        .map(PathBuf::from)
        .or_else(default_export_file)
    else {
        return Err(io::Error::other(
            "--file <contacts-export.json> is required (and the default export is not in this checkout)",
        )
        .into());
    };
    // Optional here and required in the wrapper: batch identity is `source + source_account +
    // exportId`, so a run that knows the account must say it. An empty value asks the database to
    // resolve the one account it already holds, and it refuses when there is not exactly one.
    let account = crate::apple_mail::option(args, "--source-account")
        .map(str::to_owned)
        .unwrap_or_default();

    let raw = fs::read(&file)
        .map_err(|error| io::Error::other(format!("cannot read {}: {error}", file.display())))?;
    let export: Value = serde_json::from_slice(&raw).map_err(|error| {
        io::Error::other(format!("{} is not valid JSON: {error}", file.display()))
    })?;
    let payload = export_payload(&export, &file_sha256(&raw))?;
    let contacts = payload["contacts"].as_array().map(Vec::len).unwrap_or(0);
    println!(
        "[load] {} contacts -> ODS (target={}, exportId={}, source account={})",
        contacts,
        target.as_str(),
        payload["exportId"].as_str().unwrap_or_default(),
        if account.is_empty() { "<resolved from the database>" } else { &account }
    );

    let database = crate::apple_mail::connect(target).await?;
    let landing = db::LandingDao::new(database);
    let tally = landing.load_apple_contacts(&payload, &account).await?;
    println!("{tally}");

    if tally["exitCode"].as_i64().unwrap_or(0) != 0 {
        return Err(io::Error::other(format!(
            "contacts load refused: {}",
            tally["message"]
                .as_str()
                .unwrap_or("see the tally above")
        ))
        .into());
    }
    Ok(())
}

/// `apple-sync contacts-project [dev|prod] [--source-account <account>]`
///
/// The current-state projection step of the Contacts chain, which used to be
/// `scripts/project-apple-contacts.ts`: the three statements it ran are one database function now, and
/// the transaction the script had to open by hand is the function's own.
pub async fn contacts_project(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = crate::apple_mail::target_arg(args)?;
    let account = crate::apple_mail::option(args, "--source-account").map(str::to_owned);
    println!(
        "[project] current snapshot -> l_person / l_property (target={})",
        target.as_str()
    );

    let database = crate::apple_mail::connect(target).await?;
    let landing = db::LandingDao::new(database);
    let tally = landing.project_apple_contacts(account.as_deref()).await?;
    println!("{tally}");
    Ok(())
}

/// The export the wrapper writes (`scripts/contacts-sync.sh:25`), used when `--file` is not given so
/// `pnpm contacts:load:dev` works against the checkout's own export.
fn default_export_file() -> Option<PathBuf> {
    let path = crate::apple_sync::repo_root().join("contact-export/contacts-export.json");
    path.is_file().then_some(path)
}

/// sha256 of the export file's bytes, lowercase hex — the batch's `file_sha256`.
fn file_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The payload `apple_contacts_load` expects: the export's own envelope facts plus its contacts,
/// untouched. Anything the export does not carry is a refusal here, before a database is opened — a
/// malformed export must not reach the ODS as a half-batch.
fn export_payload(export: &Value, file_sha256: &str) -> Result<Value, Box<dyn Error>> {
    let text = |key: &str| -> Result<String, Box<dyn Error>> {
        export
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| io::Error::other(format!("export has no {key}")).into())
    };

    let schema_version = export
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .ok_or_else(|| io::Error::other("export has no numeric schemaVersion"))?;
    let source_system = text("sourceSystem")?;
    if source_system != "apple_contacts" {
        return Err(io::Error::other(format!(
            "export sourceSystem is {source_system}, not apple_contacts"
        ))
        .into());
    }
    if schema_version != 1 {
        return Err(io::Error::other(format!("export schemaVersion is {schema_version}, not 1")).into());
    }
    let contacts = export
        .get("contacts")
        .and_then(Value::as_array)
        .filter(|contacts| !contacts.is_empty())
        .ok_or_else(|| io::Error::other("export contains no contacts"))?;

    Ok(serde_json::json!({
        "exportId": text("exportId")?,
        "schemaVersion": schema_version,
        "sourceSystem": source_system,
        "exportedAt": text("exportedAt")?,
        "fileSha256": file_sha256,
        "contacts": contacts,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_note_lands_on_the_contact_its_id_names() {
        let mut contact = json!({"sourceId": "A:ABPerson", "givenName": "Dana"});
        assert!(merge_note(&mut contact, "wants the west unit"));
        assert_eq!(contact["note"], json!("wants the west unit"));

        assert!(
            !merge_note(&mut contact, "   "),
            "a blank note is not a note"
        );
        assert_eq!(
            contact["note"],
            json!("wants the west unit"),
            "a blank note never overwrites the one already there"
        );
    }

    #[test]
    fn the_applescript_is_the_bulk_property_fetch() {
        // The per-person loop cost 336s over 4,662 people; this shape is what keeps it at ~5s, so
        // losing it silently would be a regression nobody would notice until a run timed out.
        assert!(APPLESCRIPT.contains("set ids to id of people"));
        assert!(APPLESCRIPT.contains("set ns to note of people"));
        assert!(!APPLESCRIPT.contains("repeat with p in people"));
    }

    fn export(overrides: Value) -> Value {
        let mut base = json!({
            "schemaVersion": 1,
            "sourceSystem": "apple_contacts",
            "exportId": "C14F508F-FCC1-4238-B06E-B97DD2E85E15",
            "exportedAt": "2026-09-22T19:33:12.308Z",
            "contacts": [{"sourceId": "A:ABPerson", "givenName": "Dana"}],
        });
        for (key, value) in overrides.as_object().unwrap() {
            base[key] = value.clone();
        }
        base
    }

    #[test]
    fn the_payload_carries_the_export_and_the_files_own_hash() {
        let payload = export_payload(&export(json!({})), "abc123").unwrap();
        assert_eq!(payload["exportId"], json!("C14F508F-FCC1-4238-B06E-B97DD2E85E15"));
        assert_eq!(payload["fileSha256"], json!("abc123"));
        assert_eq!(payload["schemaVersion"], json!(1));
        // The contacts travel exactly as exported: they are the input to the normalization the
        // database function performs, and trimming them here would move the rules back into Rust.
        assert_eq!(payload["contacts"], export(json!({}))["contacts"]);
    }

    #[test]
    fn a_malformed_export_is_refused_before_a_database_is_opened() {
        for (label, bad) in [
            ("no source system", export(json!({"sourceSystem": ""}))),
            ("wrong source system", export(json!({"sourceSystem": "apple_messages"}))),
            ("wrong schema", export(json!({"schemaVersion": 2}))),
            ("no export id", export(json!({"exportId": "  "}))),
            ("no exported at", export(json!({"exportedAt": ""}))),
            ("no contacts", export(json!({"contacts": []}))),
        ] {
            assert!(export_payload(&bad, "abc").is_err(), "{label} must fail closed");
        }
    }
}


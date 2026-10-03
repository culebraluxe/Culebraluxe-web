//! ARCH.BOUNDARY — the Vault owns document-byte authorization (TST-ARCH-BOUNDARY-008).
//!
//! Contract: a stored file's bytes are handed out through a *service door* that authorizes the actor first, and the one
//! owner of a document's bytes is the Vault. `VaultService::media_bytes` (action `vault.read`) and
//! `VaultService::public_listing_document_bytes` (action `vault.publicListingDocument.read`) are that owner; the guest
//! door additionally proves the entitlement in SQL — the media must be a `document`, a PDF, linked to a live published
//! listing exactly as a `document`, and must have **no** `transaction_document` lineage. Those clauses are what stop an
//! anonymous visitor receiving a signed contract, and they are asserted here clause by clause.
//!
//! The honest state of the tree today is that the ownership is **not exclusive**, and this test says so instead of
//! passing quietly. Two further service doors hand out a `media` row's bytes under weaker actions, and the readers they
//! — and the CLI — stand on carry **no `media_type` restriction at all**:
//!
//!   - `web/src/media/media_bytes.rs` — service door `media.bytes`, action `property.read`, over
//!     `MediaDao::media_bytes`, whose SQL is `select ... from media where id = $1` with no type filter.
//!   - `web/src/public_listings.rs` — service door `property.publicMediaBytes`, action `property.public.read`
//!     (anonymous), over `PublicListingDao::media_bytes`, which gates on the Property link but never on `media_type`.
//!   - `MediaDao::original_bytes` — a third reader with no type filter, reachable only from the CLI (`media-cards`).
//!
//! So a `document` row — including a signed contract carrying `transaction_document` lineage — is reachable through a
//! door that asks for less than the Vault asks, and (through the public door) with no actor at all. Fixing that means
//! giving each reader the `media_type` it is allowed to serve, or routing document bytes exclusively through the Vault;
//! it does not mean widening this test. Both the door set and the reader set are pinned in **both directions**: a fifth
//! door or a seventh reader fails (that is the new hole), and a pin that no longer matches fails too (so the debt may
//! only shrink, and a fix cannot land without being recorded here).
//!
//! What this test does not cover, stated so nobody reads more into a green run: it reads sources, not behaviour — it
//! does not execute the SQL, and it does not prove the Vault's own entitlement decision (`vault.read`) is correct.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__008__vault_owns_document_byte_authorization

use std::collections::BTreeSet;
use std::path::Path;

use test_harness::source;

/// Every SQL statement in `db` that reads `media.file_data`, as `path function`.
///
/// The set is pinned so a new byte reader is a deliberate entry here rather than a new way to reach a file, and so a
/// reader that disappears is noticed.
const MEDIA_BYTE_READERS: [&str; 6] = [
    "db/src/broker_signature.rs load_protected_asset",
    "db/src/media/media_row.rs media_bytes",
    "db/src/media/media_row.rs original_bytes",
    "db/src/public_listing.rs media_bytes",
    "db/src/vault/database.rs media_bytes",
    "db/src/vault/database.rs public_listing_document_bytes",
];

/// The byte readers that do **not** restrict `media_type`: the debt that lets a document's bytes out of a door that is
/// not the Vault's. Pinned so it can only shrink — and so adding the missing filter to one of them fails the test until
/// the pin is updated, which is the record that the hole was closed.
const TYPE_UNRESTRICTED_READERS: [&str; 3] = [
    "db/src/media/media_row.rs media_bytes",
    "db/src/media/media_row.rs original_bytes",
    "db/src/public_listing.rs media_bytes",
];

/// Every service method in `web/src` whose return type is a file's bytes, as
/// `path function resource action`. A door with no `authorize` call cannot appear here at all: the scanner reports an
/// empty action, which fails the comparison.
const BYTE_DOORS: [&str; 4] = [
    "web/src/media/media_bytes.rs media_bytes media property.read",
    "web/src/public_listings.rs media_bytes property property.public.read",
    "web/src/vault/mod.rs media_bytes vault vault.read",
    "web/src/vault/mod.rs public_listing_document_bytes vault vault.publicListingDocument.read",
];

/// The clauses the anonymous Vault door must keep, each one load-bearing: without them a guest could receive a signed
/// contract through the very door that exists to prevent it.
const GUEST_DOCUMENT_GUARDS: [&str; 7] = [
    "m.media_type = 'document'",
    "pm.role = 'document'",
    "p.status in ('active', 'under_contract', 'sold')",
    "p.archived_at is null",
    "p.is_published is distinct from true",
    "p.is_active_listing is distinct from true",
    "from transaction_document td",
];

/// The only files in `web/src` allowed to name the Vault's DAO: the adapter and the composition root. A route
/// or a service holding `VaultDao` is a second path to the bytes, past the Vault's own decision.
const VAULT_DAO_FILES: [&str; 2] = ["web/src/composition.rs", "web/src/vault/mod.rs"];

/// The only files allowed to name the media DAO, for the same reason.
const MEDIA_DAO_FILES: [&str; 3] = [
    "web/src/composition.rs",
    "web/src/media/media_repository.rs",
    "web/src/media/mod.rs",
];

/// One SQL statement lifted out of a source file.
struct Statement {
    /// One-based line the statement starts on, for a failure message a reader can act on.
    first_line: usize,
    /// The statement's text with whitespace collapsed and lowercased.
    text: String,
}

/// One SQL statement that reads a media row's bytes.
struct Reader {
    file: String,
    function: String,
    statement: String,
    /// Whether the statement names a `media_type` at all.
    restricted: bool,
}

/// One service method that hands a file's bytes to a caller.
struct Door {
    file: String,
    function: String,
    resource: String,
    action: String,
    /// Whether the body closes the decision with `audit_result(`.
    audited: bool,
}

// ---- scanners ----

/// A statement's text with whitespace collapsed and lowercased, so the same SQL compares equal however it is wrapped.
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The first `"…"` literal on a line of code, if it has one.
fn first_quoted(code: &str) -> Option<&str> {
    let start = code.find('"')?;
    let rest = &code[start + 1..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Every SQL statement a file's text contains, in source order.
///
/// Two shapes exist in this tree and both are read: an `r#"…"#` block — the long statements, where a lateral `select`
/// belongs to the same statement — and a one-line `"select …"` literal. A `//` comment is never read as SQL, because
/// every read starts from [`source::code_of`].
fn statements(text: &str) -> Vec<Statement> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Statement> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let code = source::code_of(lines[index]).to_string();
        let first_line = index + 1;
        if let Some(start) = code.find("r#\"") {
            let mut body = code[start + 3..].to_string();
            while !body.contains("\"#") {
                index += 1;
                if index >= lines.len() {
                    break;
                }
                body.push('\n');
                body.push_str(source::code_of(lines[index]));
            }
            if let Some(end) = body.find("\"#") {
                body.truncate(end);
            }
            out.push(Statement {
                first_line,
                text: normalized(&body),
            });
        } else if let Some(literal) = first_quoted(&code) {
            out.push(Statement {
                first_line,
                text: normalized(literal),
            });
        }
        index += 1;
    }
    out
}

/// Whether a statement returns bytes out of the `media` table.
///
/// The bytes must be in the **select list** — the text before the first `from`, whitespace-collapsed — because a
/// statement that merely *mentions* `file_data` in a predicate (`where … file_data is not null`) does not hand a file
/// to anyone. `join media` counts as well as `from media`: a reader that reached the table through a join would
/// otherwise be invisible.
fn is_media_byte_reader(statement: &str) -> bool {
    if !statement.starts_with("select ") {
        return false;
    }
    let columns = statement.split(" from ").next().unwrap_or(statement);
    columns.contains("file_data")
        && (statement.contains("from media") || statement.contains("join media"))
}

/// The name of the function a one-based line sits inside, or `<none>` when nothing encloses it.
fn enclosing_fn(text: &str, first_line: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    for index in (0..first_line.saturating_sub(1).min(lines.len())).rev() {
        let code = source::code_of(lines[index]);
        let Some(position) = code.find("fn ") else {
            continue;
        };
        let rest = &code[position + 3..];
        let name: String = rest
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() && matches!(rest[name.len()..].chars().next(), Some('(' | '<')) {
            return name;
        }
    }
    "<none>".to_string()
}

/// Every statement under `root` that reads a media row's bytes.
fn scan_readers(root: &Path) -> Vec<Reader> {
    let mut out = Vec::new();
    for path in source::sources_under(root) {
        let text = source::read(&path);
        for statement in statements(&text) {
            if !is_media_byte_reader(&statement.text) {
                continue;
            }
            out.push(Reader {
                file: source::relative(&path),
                function: enclosing_fn(&text, statement.first_line),
                restricted: statement.text.contains("media_type"),
                statement: statement.text,
            });
        }
    }
    out
}

/// The `(resource, action)` an `authorize(` call names, as the first two string literals after it.
///
/// Both shapes this tree can hold are read: the literals on the following lines (one argument per line) and the whole
/// call written on one line. Anything before the `authorize(` is not read, so an `OP` constant declared above the call
/// cannot be mistaken for the resource.
fn authorized_pair(body: &str) -> (String, String) {
    let mut literals: Vec<String> = Vec::new();
    let mut opened = false;
    for line in body.lines() {
        let code = source::code_of(line);
        let mut tail = if opened {
            code
        } else if let Some(position) = code.find("authorize(") {
            opened = true;
            &code[position + "authorize(".len()..]
        } else {
            continue;
        };
        while literals.len() < 2 {
            let Some(start) = tail.find('"') else {
                break;
            };
            let rest = &tail[start + 1..];
            let Some(end) = rest.find('"') else {
                break;
            };
            literals.push(rest[..end].to_string());
            tail = &rest[end + 1..];
        }
        if literals.len() >= 2 {
            break;
        }
    }
    (
        literals.first().cloned().unwrap_or_default(),
        literals.get(1).cloned().unwrap_or_default(),
    )
}

/// Whether a line is the return type of a method that hands a file's bytes to a caller.
///
/// A door can answer *no such file*, so it returns `Option`; a pure builder that computes bytes always returns them and
/// is not a door at all (`vault/pdf.rs finish` returns `Result<Vec<u8>, PdfError>` and is deliberately not pinned).
/// The two door shapes this tree uses are `Result<Option<(String, Vec<u8>)>, CoreServiceError>` and
/// `Result<Option<VaultMediaBytes>, CoreServiceError>`, written on one line with the parameters one per line — which is
/// also what keeps the DAO's own `DbResult<Option<…>>` declarations out of the set. A door written another way would be
/// missed here and reported by the pin below as a missing entry rather than passing unnoticed.
fn is_byte_return(code: &str) -> bool {
    let trimmed = code.trim();
    trimmed.starts_with(')')
        && trimmed.contains("-> Result<Option<")
        && (trimmed.contains("Vec<u8>") || trimmed.contains("VaultMediaBytes"))
}

/// Every byte door in one file's text.
fn doors_in(text: &str, file: &str) -> Vec<Door> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !is_byte_return(source::code_of(line)) {
            continue;
        }
        let mut body = String::new();
        for (offset, body_line) in lines.iter().skip(index).enumerate() {
            body.push_str(body_line);
            body.push('\n');
            if offset > 0 && *body_line == "    }" {
                break;
            }
        }
        let (resource, action) = authorized_pair(&body);
        out.push(Door {
            file: file.to_string(),
            function: enclosing_fn(text, index + 1),
            resource,
            action,
            audited: body.contains("audit_result("),
        });
    }
    out
}

/// Every byte door under `root`.
fn scan_doors(root: &Path) -> Vec<Door> {
    let mut out = Vec::new();
    for path in source::sources_under(root) {
        let text = source::read(&path);
        out.extend(doors_in(&text, &source::relative(&path)));
    }
    out
}

/// The files under `root` whose *code* (not comments) names `needle` as an identifier segment.
fn files_naming(root: &Path, needle: &str) -> Vec<String> {
    let mut out = Vec::new();
    for path in source::sources_under(root) {
        let text = source::read(&path);
        if text
            .lines()
            .any(|line| source::contains_segment(source::code_of(line), needle))
        {
            out.push(source::relative(&path));
        }
    }
    out
}

/// The two halves of a ratchet: pins no longer found, and findings not pinned.
fn ratchet_delta(pinned: &[&str], actual: &[String]) -> (Vec<String>, Vec<String>) {
    let seen: BTreeSet<String> = actual.iter().cloned().collect();
    let known: BTreeSet<String> = pinned.iter().map(|entry| entry.to_string()).collect();
    (
        known.difference(&seen).cloned().collect(),
        seen.difference(&known).cloned().collect(),
    )
}

/// Assert a pin still describes the tree exactly, in both directions.
fn ratchet(pinned: &[&str], actual: &[String], what: &str) {
    let (missing, extra) = ratchet_delta(pinned, actual);
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{what} changed: {} pinned but not found [{}]; {} found but not pinned [{}]. \
         The pins are the contract — a new entry is a new hole, and an entry that vanished means either the debt was \
         paid (update the pin in the same commit) or the detector stopped seeing it.",
        missing.len(),
        missing.join(", "),
        extra.len(),
        extra.join(", ")
    );
}

/// A door as the pins name it.
fn door_key(door: &Door) -> String {
    format!(
        "{} {} {} {}",
        door.file, door.function, door.resource, door.action
    )
}

/// A reader as the pins name it.
fn reader_key(reader: &Reader) -> String {
    format!("{} {}", reader.file, reader.function)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-008); the file and the assay use it.
fn arch_boundary_008__vault_owns_document_byte_authorization() {
    let db_root = source::workspace_root().join("db/src");
    let server_root = source::workspace_root().join("web/src");

    // 0. The walkers found a tree. Every assertion below is silent about a file nobody read, so the floor comes first.
    let db_files = source::sources_under(&db_root);
    let server_files = source::sources_under(&server_root);
    assert!(
        db_files.len() >= 60,
        "the database sources must be walked; {} files found",
        db_files.len()
    );
    assert!(
        server_files.len() >= 80,
        "the server sources must be walked; {} files found",
        server_files.len()
    );

    let readers = scan_readers(&db_root);
    let doors = scan_doors(&server_root);
    assert!(
        readers.len() >= 5,
        "the byte-reader detector must find the readers this contract is about; {} found",
        readers.len()
    );
    assert!(
        doors.len() >= 3,
        "the byte-door detector must find the doors this contract is about; {} found",
        doors.len()
    );

    // 1. Every statement that reads a media row's bytes is pinned, so a seventh reader is a decision, not a leak.
    ratchet(
        &MEDIA_BYTE_READERS,
        &readers.iter().map(reader_key).collect::<Vec<_>>(),
        "the set of media byte readers",
    );

    // 2. A reader's id is a bind, never interpolated: a value must never be able to become SQL.
    for reader in &readers {
        assert!(
            reader.statement.contains("$1"),
            "{} `{}` must take its id as a bound parameter",
            reader.file,
            reader.function
        );
        assert!(
            !reader.statement.contains('{'),
            "{} `{}` must not hand-build SQL",
            reader.file,
            reader.function
        );
    }

    // 3. THE VAULT'S OWN READERS SERVE DOCUMENTS ONLY, and the guest reader proves the entitlement in SQL. This is the
    //    authorization the contract names, so it is asserted clause by clause rather than as "the file mentions it".
    let vault_readers: Vec<&Reader> = readers
        .iter()
        .filter(|reader| reader.file == "db/src/vault/database.rs")
        .collect();
    assert_eq!(
        vault_readers.len(),
        2,
        "the Vault owns exactly two byte readers (the private and the anonymous one)"
    );
    for reader in &vault_readers {
        assert!(
            reader.restricted && reader.statement.contains("media_type = 'document'"),
            "vault/{} must serve documents only",
            reader.function
        );
    }
    let private = vault_readers
        .iter()
        .find(|reader| reader.function == "media_bytes")
        .expect("the private reader is pinned in MEDIA_BYTE_READERS");
    assert!(
        private.statement.contains("file_data is not null"),
        "the private reader must not return an empty file as a document"
    );
    let guest = vault_readers
        .iter()
        .find(|reader| reader.function == "public_listing_document_bytes")
        .expect("the anonymous reader is pinned in MEDIA_BYTE_READERS");
    for guard in GUEST_DOCUMENT_GUARDS {
        assert!(
            guest.statement.contains(&normalized(guard)),
            "the anonymous Vault door must keep its guard `{guard}`: without it a guest can receive a file the Vault \
             exists to withhold"
        );
    }
    assert!(
        guest.statement.contains("'application/pdf'"),
        "the anonymous Vault door must serve PDFs only"
    );

    // 4. WHICH READERS SERVE ANY MEDIA TYPE. That set is the debt: those readers can hand out a `document` — a signed
    //    contract included — without the Vault's decision ever being asked.
    let unrestricted: Vec<String> = readers
        .iter()
        .filter(|reader| !reader.restricted)
        .map(reader_key)
        .collect();
    ratchet(
        &TYPE_UNRESTRICTED_READERS,
        &unrestricted,
        "the set of media byte readers with no media_type restriction",
    );
    for reader in readers.iter().filter(|reader| !reader.restricted) {
        assert!(
            !reader.file.starts_with("db/src/vault/"),
            "{} `{}` serves any media type from inside the Vault",
            reader.file,
            reader.function
        );
    }

    // 5. Every service door that hands out bytes is pinned, and each one asks for a named action and audits the
    //    decision. A door with no action is not a door: it is a file served to whoever asked.
    ratchet(
        &BYTE_DOORS,
        &doors.iter().map(door_key).collect::<Vec<_>>(),
        "the set of service doors that hand out file bytes",
    );
    for door in &doors {
        assert!(
            !door.resource.is_empty() && !door.action.is_empty(),
            "{} `{}` hands out bytes without an authorize() call naming a resource and an action",
            door.file,
            door.function
        );
        assert!(
            door.audited,
            "{} `{}` must close its authorization decision with audit_result",
            door.file, door.function
        );
    }

    // 6. The Vault's doors are the two that ask for the Vault's own actions.
    let vault_doors: Vec<&Door> = doors
        .iter()
        .filter(|door| door.resource == "vault")
        .collect();
    assert_eq!(
        vault_doors.len(),
        2,
        "the Vault owns exactly two byte doors: the document's bytes and the anonymous listing document's"
    );
    assert_eq!(
        vault_doors
            .iter()
            .filter(|door| door.action == "vault.read")
            .count(),
        1,
        "opening a document's bytes must ask for `vault.read`"
    );
    assert_eq!(
        vault_doors
            .iter()
            .filter(|door| door.action == "vault.publicListingDocument.read")
            .count(),
        1,
        "the anonymous door must ask for its own action, never the member's `vault.read`"
    );

    // 7. THE DEBT, NAMED. The doors that are not the Vault's are the weaker ones, and the two `media` readers beneath
    //    them do not restrict `media_type` — so a document's bytes are reachable without the Vault's decision. This
    //    assertion exists to make that a fact the suite states, rather than a thing a reader has to notice.
    let weak_doors = [
        "web/src/media/media_bytes.rs media_bytes",
        "web/src/public_listings.rs media_bytes",
    ];
    for door in doors.iter().filter(|door| door.resource != "vault") {
        let key = format!("{} {}", door.file, door.function);
        assert!(
            weak_doors.contains(&key.as_str()),
            "{} `{}` hands out bytes under `{}` and is not one of the two known weaker doors; a new way to a file is \
             a new hole, not a convenience",
            door.file,
            door.function,
            door.action
        );
    }
    let weak_readers: Vec<String> = unrestricted
        .iter()
        .filter(|key| key.ends_with(" media_bytes"))
        .cloned()
        .collect();
    assert_eq!(
        weak_readers.len(),
        2,
        "the two non-vault doors must sit on readers that serve any media type — that is the debt this test records; \
         found {weak_readers:?}"
    );

    // 8. Only the adapter and the composition root may name the DAOs: a route or a screen holding one would reach the
    //    bytes without a service's authorization ever running.
    ratchet(
        &VAULT_DAO_FILES,
        &files_naming(&server_root, "VaultDao"),
        "the files that name VaultDao",
    );
    ratchet(
        &MEDIA_DAO_FILES,
        &files_naming(&server_root, "MediaDao"),
        "the files that name MediaDao",
    );

    // 9. NEGATIVE CONTROLS. Each detector is run against a planted sample, so a green run is evidence the detector
    //    works rather than evidence it found nothing.
    let planted = "fn probe() {\n    sqlx::query(r#\"\n        select file_data from media where id = $1::uuid\n    \"#);\n}\n";
    let found = statements(planted);
    assert_eq!(
        found.len(),
        1,
        "a raw string is one statement, however many lines it spans"
    );
    assert!(
        is_media_byte_reader(&found[0].text) && !found[0].text.contains("media_type"),
        "a planted reader with no type restriction must be seen, and seen as unrestricted"
    );
    assert_eq!(
        enclosing_fn(planted, found[0].first_line),
        "probe",
        "a reader must be attributed to the function it sits in"
    );
    let guarded = planted.replace(
        "where id = $1::uuid",
        "where id = $1::uuid and media_type = 'image'",
    );
    assert!(
        statements(&guarded)[0].text.contains("media_type"),
        "a planted reader that does restrict the type must be seen as restricted"
    );
    let writer =
        "fn probe() {\n    sqlx::query(\"insert into media (file_data) values ($1)\");\n}\n";
    assert!(
        statements(writer)
            .iter()
            .all(|statement| !is_media_byte_reader(&statement.text)),
        "a writer must not be counted as a reader"
    );
    let predicate = "fn probe() {\n    sqlx::query(r#\"\n        select m.id::text, m.filename\n          from media m\n         where m.file_data is not null\n    \"#);\n}\n";
    assert!(
        statements(predicate)
            .iter()
            .all(|statement| !is_media_byte_reader(&statement.text)),
        "a statement that only checks `file_data is not null` returns no file and is not a reader"
    );
    let commented = "fn probe() {\n    // select file_data from media\n}\n";
    assert!(
        statements(commented)
            .iter()
            .all(|statement| !is_media_byte_reader(&statement.text)),
        "a sentence about a reader must not be read as one"
    );

    let leaky = "impl X {\n    pub async fn leaky(\n        &self,\n    ) -> Result<Option<(String, Vec<u8>)>, CoreServiceError> {\n        let result = Ok(None);\n        result\n    }\n}\n";
    let leaky_doors = doors_in(leaky, "planted.rs");
    assert_eq!(leaky_doors.len(), 1, "a byte-returning method must be seen");
    assert!(
        leaky_doors[0].resource.is_empty() && leaky_doors[0].action.is_empty(),
        "a door with no authorize() call must report an empty action, which is what makes it fail the pin"
    );
    assert!(
        !leaky_doors[0].audited,
        "a door with no audit_result must be seen as unaudited"
    );
    let trailed = leaky.replace(
        "        result\n",
        "        let decision = authorize(&self.runtime, \"vault\", \"vault.read\", OP, OperationKind::Query, context).await?;\n        result\n",
    );
    let trailed_doors = doors_in(&trailed, "planted.rs");
    assert!(
        trailed_doors[0].resource == "vault" && trailed_doors[0].action == "vault.read",
        "the resource and the action must be read from the authorize() call, not guessed"
    );
    let builder = "impl X {\n    pub fn finish(\n        &mut self,\n    ) -> Result<Vec<u8>, PdfError> {\n        let out = Vec::new();\n        Ok(out)\n    }\n}\n";
    assert!(
        doors_in(builder, "planted.rs").is_empty(),
        "a builder that always returns the bytes it computed is not a door and must not be pinned as one"
    );

    assert_eq!(
        ratchet_delta(&["a", "b"], &["a".to_string(), "c".to_string()]),
        (vec!["b".to_string()], vec!["c".to_string()]),
        "the ratchet must report both directions: a pin that vanished and a finding that was never pinned"
    );
}

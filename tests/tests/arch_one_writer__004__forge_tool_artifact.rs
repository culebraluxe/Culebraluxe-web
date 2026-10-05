//! ARCH.ONE_WRITER — `forge_tool_artifact` has exactly ONE writer (TST-ARCH-ONE_WRITER-004).
//!
//! CONTRACT. `AGENTS.md`: "Let two sources answer one fact. One fact has ONE writer; if two ever disagree,
//! that is a REFUSAL (HOLD) naming both...". The production guard (`cli/src/forge/repo_guards.rs`,
//! `writer_scan`) freezes the writer SET per audited table; this test is a second, independent reading of
//! the same tree with its own scanner — no import of the guard, no `regex` crate — on a table the guard
//! does **not** audit (`AUDITED_TABLES` is `storyboard_story`, `storyboard_story_run`,
//! `forge_workflow_evidence`). `forge_tool_artifact` is the third of the three rows `AGENTS.md` names as
//! "the rows", so its fence is written here.
//!
//! THREE FACTS, pinned in **both directions** — a new writer fails here (that is the point) and a pin the
//! tree no longer matches also fails, so the set can only move by a deliberate edit of this file:
//!
//!   1. THE WRITE IS ONE FILE, AND IT DOES NOT SPELL THE WRITE. No production `.rs` file contains
//!      `update forge_tool_artifact`, `insert into forge_tool_artifact` or `delete from forge_tool_artifact`
//!      at all — that set is pinned **empty**: migration 267 moved the INSERT out of Rust into
//!      `forge_record_tool_artifact()`, and `db/src/forge_engine.rs` (whose doc comment still says "the one
//!      write", reading migration 130) now executes `select … from forge_record_tool_artifact($1, …)` and
//!      binds parameters. So the file that writes the table is the file that invokes the door, and the
//!      union of the two detectors is **exactly one file, `db/src/forge_engine.rs`** — the strongest form
//!      of this contract, the same one the guard asserts for `forge_workflow_evidence`. A second door (a
//!      new direct speller, or a second file naming `forge_record_tool_artifact`) fails.
//!   2. THE TABLE, ITS NEIGHBOURS AND THE FUNCTION'S OWN NAME ARE THREE DIFFERENT THINGS. The DAO's READ
//!      of the table (`select detail from forge_tool_artifact …`, `candidate_code_for_story`) is not a
//!      write; `update forge_workflow_evidence set …` is not this table; `update forge_tool_artifact_
//!      archive set …` is not either (identifier boundary after the name); `select id from
//!      forge_record_tool_artifact(…)` is not a *direct* spelling — `forge_record_tool_artifact` does not
//!      contain the identifier `forge_tool_artifact` — while invoking it IS the door; and `writer.
//!      record_tool_artifact(&artifact)`, the Rust API method the roles call, is neither (it is a method
//!      name, not the database function). A `///` comment quoting a statement is not code. Every
//!      direction is planted below, because a scanner that cannot fail cannot pass.
//!   3. THE BLIND SPOT, DERIVED AND SMALL. Every `create … function` region of `db/migrations` is run
//!      through the same detector: exactly ONE function body writes the table —
//!      `forge_record_tool_artifact` (267) — and there are **zero** apply-time one-offs (130 only creates
//!      the table). The function's production callers are pinned to the single file above, and the table's
//!      creation is asserted to still exist in the ledger, so the subject cannot quietly disappear.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * SCOPE. The scan reads `.rs` files under `web/`, `middle/`, `db/`, `cli/`, `forge/` only. `tests/` is
//!     outside it — this file's own planted samples quote `insert into forge_tool_artifact …`, and the
//!     exclusion is asserted (not assumed): if the filter broke, the scan would flag this test as a writer
//!     and fail. `cli/src/forge/repo_guards.rs` is excluded the way it excludes itself (its fixtures quote
//!     writes of other tables). `db/migrations/*.sql` is not `.rs` and `experiments/` is outside the
//!     roots, so neither is scanned; `legacy/` is retired TypeScript and is deliberately not read (Rust
//!     only).
//!   * ROOT. The scan's root is resolved inside this file (`env!("CARGO_MANIFEST_DIR")`, the same
//!     expression `test_harness::source::repo_root()` uses) instead of through the helper. The cargo
//!     target dir (`build/rust`) is shared by every lane of this repo and `tests/src/*.rs` is
//!     byte-identical in all of them, so the `test_harness` rlib can be a sibling lane's artifact —
//!     and a sibling's `env!` bakes THAT lane's checkout in (observed 2026-10-04: a run in `lane-mimo`
//!     linked a `lane-nemotron` rlib and would have measured the wrong tree). The helpers that take an
//!     explicit path (`source::sources_under`, `source::read`) are used as they are; only the root is
//!     local, and the SELF check below proves it resolves to this checkout.
//!   * AGREEMENT. The guard has no `forge_tool_artifact` row, so there is nothing to agree with — this
//!     test adds the fence. `docs/agent/MAP-engine.md` does claim one ("**One writer**:
//!     `ForgeEngineDao::record_tool_artifact` (migration 130)") and this reading agrees with the ONE-writer
//!     claim; the migration number has moved, 130 creates the table and 267 carries the write the DAO
//!     calls. Read by eye on 2026-10-04 — if the two ever diverge, the divergence is the finding.
//!   * DELETE. The guard's `writes_table` counts `update` and `insert into`; this reading adds `delete
//!     from`, because a delete writes the fact too. No production file and no migration deletes the table,
//!     so the two readings agree in result — and if one ever appears, it fails here rather than hiding
//!     behind the narrower definition.
//!   * WHO TRIGGERS IT, counted but not pinned. The Rust recording API (`ForgeStateWriter::
//!     record_tool_artifact`, implemented in `forge/src/engine/db_writer.rs`, declared in
//!     `forge/src/engine/writer.rs`) is called by `forge/src/roles/smith.rs` and `forge/src/roles/qa.rs`.
//!     Those files cause a row to exist but do not write the table: the executed statement lives in one
//!     file, which is exactly the reading the guard applies to `forge_workflow_evidence` (many files cause
//!     evidence rows; it is pinned to `db/src/forge_engine.rs`, the only file with the INSERT). Recording
//!     an artifact is those roles' job and stays theirs.
//!   * THE AUDIT DOC. `docs/agent/COLUMN-WRITER-AUDIT.md` covers `storyboard_story`,
//!     `storyboard_story_run` and `forge_workflow_evidence` — there is no `forge_tool_artifact` section,
//!     so there is no column row to agree or disagree with. Its rows are history (retired TypeScript
//!     writers), never this scan's input.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it reads sources, not
//! behaviour. It does not prove the polarity rule inside `forge_record_tool_artifact` keeps or drops the
//! right verdicts, does not run a statement, and does not see a write spelled inside a migration function
//! body or fired by a trigger (fact 3 bounds the Rust side of that blind spot — the door has one caller —
//! and nothing more).
//!
//! Level: L0 Pure — filesystem reads only. No database, no network, no process spawned.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__004__forge_tool_artifact

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use test_harness::source;

/// The file doing the scanning. Its planted samples quote the statement it hunts, so the scope filter must
/// never count it — asserted below rather than assumed.
const SELF: &str = "tests/tests/arch_one_writer__004__forge_tool_artifact.rs";

/// The roots a production writer may live under: the tiers (`web/`, `middle/`, `db/`) and the entry points
/// (`cli/`, `forge/`). `tests/` is deliberately absent — that crate is the fixture client and this suite.
const PRODUCTION_ROOTS: [&str; 5] = ["web/", "middle/", "db/", "cli/", "forge/"];

/// The production guard's source, excluded the way it excludes itself (`is_guard_source`).
const GUARD_SOURCE: &str = "cli/src/forge/repo_guards.rs";

/// The table under contract, and the table it is most easily confused with (both are Forge run/evidence
/// rows; fact 2 plants both directions).
const TABLE: &str = "forge_tool_artifact";
const NEIGHBOUR_TABLE: &str = "forge_workflow_evidence";

/// The database function that IS the write (migration 267) — the door every Rust file must go through.
const DOOR: &str = "forge_record_tool_artifact";

/// Production `.rs` files that spell the write directly (`update`/`insert into`/`delete from
/// forge_tool_artifact`). Frozen EMPTY: the statement moved into the database in migration 267, and a file
/// that spells it again is a second door past the polarity rule the stored function enforces.
const DIRECT_WRITERS: [&str; 0] = [];

/// The production `.rs` files that write the table through [`DOOR`]. One, and it stays one — the strongest
/// form of this contract, asserted below as `found.len() == 1` as well as by set equality.
const DOOR_CALLERS: [&str; 1] = ["db/src/forge_engine.rs"];

/// Each pinned file, and text its RAW source must carry — the scan would otherwise pass by reading the
/// wrong file.
const READ_PROOF: [(&str, &str); 1] =
    [("db/src/forge_engine.rs", "from forge_record_tool_artifact(")];

/// The migration functions whose bodies write the table, and the production `.rs` files allowed to invoke
/// each one. Derived from `db/migrations` by the test (every `create … function` region run through the
/// same detector), so a NEW artifact-writing function fails here instead of slipping past the pin.
const DB_WRITE_FUNCTIONS: [(&str, &[&str]); 1] = [(DOOR, &["db/src/forge_engine.rs"])];

/// The apply-time one-offs: migrations whose nameless region writes the table. None has ever existed —
/// migration 130 only creates it — and a second door written as a bare statement fails the pin.
const SQL_ONE_OFFS: [&str; 0] = [];

/// Floors. A walker that found nothing would report a clean tree and pass; these make that a failure.
const RUST_SOURCE_FLOOR: usize = 700;
const PRODUCTION_RUST_FLOOR: usize = 500;

fn in_repo(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

/// The repository root this compilation unit was built from: `tests/`, one level below the root.
///
/// The same expression `test_harness::source::repo_root()` uses — evaluated HERE rather than called there
/// on purpose. The cargo target dir (`build/rust`) is shared by every lane of this repo, `tests/src/*.rs`
/// is byte-identical in all of them, so the `test_harness` rlib may be a sibling lane's artifact, and a
/// sibling's `env!("CARGO_MANIFEST_DIR")` bakes THAT lane's path in: reading the tree through it measures
/// another checkout (observed 2026-10-04: a run linked a `lane-nemotron` rlib while building in
/// `lane-mimo`). The helpers that take an explicit path (`source::sources_under`, `source::read`) are used
/// as-is; only the root is resolved here. The cross-check in the test body pins this to the invocation.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the suite lives in tests/, one level below the repository root")
        .to_path_buf()
}

/// `path` as this tree writes it — relative to the root, for failure messages a reader can act on.
/// `source::relative` does this too, but against `source::repo_root()`'s (possibly foreign) root.
fn relative_to_root(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Whether `relative` is a production `.rs` file this scan reads. The scope rule, in one place.
fn is_scanned_path(relative: &str) -> bool {
    relative.ends_with(".rs")
        && PRODUCTION_ROOTS
            .iter()
            .any(|root| relative.starts_with(root))
        && !relative.contains("/tests/")
        && !relative.ends_with("_test.rs")
        && relative != GUARD_SOURCE
}

/// The CODE in `source_text`: `//` and `/* … */` comments removed, string and char literals copied whole
/// (the SQL lives in the literals, so a stripper that swallowed them would find no writer at all).
///
/// Written by hand rather than reusing `source::code_of` because that reads a `//` inside a string literal
/// as a comment — `"https://…"` would swallow the rest of the line. Planted samples check the behaviour.
fn code_of_file(source_text: &str) -> String {
    let mut out = String::with_capacity(source_text.len());
    let mut characters = source_text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '/' if characters.peek() == Some(&'/') => {
                for next in characters.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if characters.peek() == Some(&'*') => {
                characters.next();
                let mut previous = ' ';
                for next in characters.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    if next == '\n' {
                        out.push('\n');
                    }
                    previous = next;
                }
                out.push(' ');
            }
            '"' => {
                out.push(character);
                copy_string(&mut characters, &mut out);
            }
            '\'' => {
                out.push(character);
                copy_char_literal(&mut characters, &mut out);
            }
            'r' if matches!(characters.peek(), Some('"') | Some('#')) => {
                out.push(character);
                let mut hashes = 0usize;
                while characters.peek() == Some(&'#') {
                    characters.next();
                    out.push('#');
                    hashes += 1;
                }
                if characters.peek() == Some(&'"') {
                    characters.next();
                    out.push('"');
                    let terminator = format!("\"{}", "#".repeat(hashes));
                    let mut window = String::new();
                    for next in characters.by_ref() {
                        window.push(next);
                        out.push(next);
                        if window.ends_with(&terminator) {
                            break;
                        }
                        if window.len() > terminator.len() {
                            window.remove(0);
                        }
                    }
                }
            }
            _ => out.push(character),
        }
    }
    out
}

/// Copy a `"…"` literal whole, escapes included.
fn copy_string(characters: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    while let Some(character) = characters.next() {
        out.push(character);
        if character == '\\' {
            if let Some(escaped) = characters.next() {
                out.push(escaped);
            }
        } else if character == '"' {
            return;
        }
    }
}

/// Copy a `'…'` char literal whole. A lifetime (`'static`) is left alone: treating its apostrophe as an
/// opening quote would eat the file.
fn copy_char_literal(characters: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    let lookahead: String = characters.clone().take(4).collect();
    let Some(closing) = lookahead.find('\'') else {
        return;
    };
    if closing > 2 {
        return;
    }
    for _ in 0..=closing {
        if let Some(character) = characters.next() {
            out.push(character);
        }
    }
}

/// Lowercased, whitespace-collapsed text. SQL keywords may sit on any line of a multi-line literal, and
/// `UPDATE … SET` is written both cases in this tree — collapsing makes both spellings one match. Byte
/// length is preserved by `to_ascii_lowercase`, and no caller indexes the collapsed text against the
/// original, so the rebuild is safe.
fn normalized(code: &str) -> String {
    code.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every position of `needle` in `haystack` whose surroundings are identifier boundaries, so
/// `forge_tool_artifact` never answers for `forge_tool_artifact_archive` (and vice versa).
fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .map(|c| !c.is_alphanumeric() && c != '_')
            .unwrap_or(true);
        let after_ok = haystack[end..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric() && c != '_')
            .unwrap_or(true);
        if before_ok && after_ok {
            out.push(start);
        }
        from = start + 1;
    }
    out
}

/// Detector 1 of 2: does this code EXECUTE a statement that writes `table`? Three verbs, because all
/// three move the fact — `update`, `insert into`, `delete from` (the guard counts the first two; see the
/// DELETE note in the header for why the third is here and why the readings agree).
fn writes_table(code: &str, table: &str) -> bool {
    let normalized_text = normalized(code);
    ["update ", "insert into ", "delete from "]
        .iter()
        .any(|verb| !find_all(&normalized_text, &format!("{verb}{table}")).is_empty())
}

/// Detector 2 of 2: does this code invoke the database function that IS the write? Spelled as a name
/// lookup, so the function's own name must appear as an identifier — `record_tool_artifact` (the Rust API
/// method) does not carry the `forge_` prefix and does not answer.
fn invokes_door(code: &str, function: &str) -> bool {
    !find_all(&normalized(code), function).is_empty()
}

/// Every production `.rs` file, as (relative path, code with comments stripped). Deterministic order.
fn production_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in source::sources_under(&repo_root()) {
        let relative = relative_to_root(&path);
        if is_scanned_path(&relative) {
            out.push((relative, code_of_file(&source::read(&path))));
        }
    }
    out
}

/// The two detectors' union over an already-read file list: the files that may write `table`.
fn writers_of(files: &[(String, String)], table: &str, door: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| writes_table(code, table) || invokes_door(code, door))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The production files that spell the write directly — the first detector alone, so its pin can stay
/// empty without the door detector hiding a new speller.
fn direct_writers_of(files: &[(String, String)], table: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| writes_table(code, table))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The production files whose (comment-stripped) code names `function` — the callers of a database
/// function, found without trusting anyone's list of them.
fn callers_of(files: &[(String, String)], function: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| !find_all(code, function).is_empty())
        .map(|(path, _)| path.clone())
        .collect()
}

fn pinned(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|path| path.to_string()).collect()
}

/// Every `.sql` file in `db/migrations`, read as text — the blind-spot side of fact 3.
fn migration_texts() -> Vec<(String, String)> {
    let dir = in_repo("db/migrations");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        panic!(
            "{} must be readable — the migration ledger is part of this contract",
            dir.display()
        );
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push((name, source::read(&path)));
    }
    out.sort();
    out
}

/// A migration's SQL with `--` comments removed: a comment that merely QUOTES a statement must never
/// satisfy a check that the statement exists.
fn sql_code(sql: &str) -> String {
    sql.lines()
        .map(|line| line.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One migration split into its `create … function` regions, as (function name, region text). A nameless
/// region is text outside any function — apply-time statements. Regions are lowercased with comments
/// stripped first, so a commented-out `create function` cannot fabricate a door.
///
/// Deliberate imprecision, recorded: statements written AFTER a file's last function belong to that
/// function's region. Such a misattribution can only ADD a name to the enumeration, which then fails
/// loudly against the pin rather than hiding a writer.
fn sql_function_regions(sql: &str) -> Vec<(String, String)> {
    let text = sql_code(sql).to_lowercase();
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for marker in ["create or replace function ", "create function "] {
        let mut from = 0usize;
        while let Some(offset) = text[from..].find(marker) {
            let at = from + offset;
            cuts.push((at, marker.len()));
            from = at + marker.len();
        }
    }
    cuts.sort();
    if cuts.is_empty() {
        return vec![(String::new(), text)];
    }
    let mut regions = Vec::new();
    if cuts[0].0 > 0 {
        regions.push((String::new(), text[..cuts[0].0].to_string()));
    }
    let mut previous_end = cuts[0].0 + cuts[0].1;
    for index in 1..cuts.len() {
        let (start, len) = cuts[index];
        let region = &text[previous_end..start];
        regions.push((function_name_of(region), region.to_string()));
        previous_end = start + len;
    }
    let region = &text[previous_end..];
    regions.push((function_name_of(region), region.to_string()));
    regions
}

/// The function name a region defines: the word before its first `(`, or "" when there is none.
fn function_name_of(region: &str) -> String {
    let Some(open) = region.find('(') else {
        return String::new();
    };
    region[..open]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

/// The blind-spot enumeration, in one pass: the CURRENT (last-defined, filename order) migration
/// functions whose bodies write the table through `writes`, plus the files whose nameless regions do —
/// the apply-time one-offs. A restatement that stops writing drops the name; a new artifact door adds
/// one; both directions fail the pin.
fn sql_writer_regions<F>(
    migrations: &[(String, String)],
    writes: F,
) -> (BTreeSet<String>, BTreeSet<String>)
where
    F: Fn(&str) -> bool,
{
    let mut current: BTreeMap<String, bool> = BTreeMap::new();
    let mut one_offs: BTreeSet<String> = BTreeSet::new();
    for (file, sql) in migrations {
        for (function, region) in sql_function_regions(sql) {
            let writes_now = writes(&region);
            if function.is_empty() {
                if writes_now {
                    one_offs.insert(file.clone());
                }
            } else {
                current.insert(function, writes_now);
            }
        }
    }
    let functions = current
        .into_iter()
        .filter(|(_, writes_now)| *writes_now)
        .map(|(name, _)| name)
        .collect();
    (functions, one_offs)
}

#[test]
#[allow(non_snake_case)]
fn arch_one_writer_004__forge_tool_artifact() {
    // ── 1. THE DETECTOR, against planted samples. A scan that cannot fail is not a check. ─────────────────

    // Detector 1 (a direct spelling of the write) sees every verb — production spells NONE of them, so the
    // planted samples are the only thing proving the empty pin below is a measurement and not a shrug.
    assert!(
        writes_table(
            "sqlx::query(\"insert into forge_tool_artifact (story_id, story_run_id, tool, kind, verdict, summary, detail, sha) values ($1,$2,$3,$4,$5,$6,$7,$8)\")",
            TABLE
        ),
        "a planted `insert into forge_tool_artifact` was not seen"
    );
    assert!(
        writes_table(
            "sqlx::query(\"update forge_tool_artifact set verdict='PASS' where id=$1\")",
            TABLE
        ),
        "a planted `update forge_tool_artifact` was not seen"
    );
    assert!(
        writes_table(
            "sqlx::query(\"delete from forge_tool_artifact where story_run_id=$1\")",
            TABLE
        ),
        "a planted `delete from forge_tool_artifact` was not seen — a delete writes the fact too"
    );

    // A READ of the table is not a write of it — the DAO's own read (`candidate_code_for_story`).
    assert!(
        !writes_table(
            "sqlx::query_scalar::<_, Option<Value>>(\"select detail from forge_tool_artifact where story_id=$1 and kind='candidate-code'\")",
            TABLE
        ),
        "the DAO's SELECT of the table was read as a write of it"
    );

    // TABLE precision: a neighbouring Forge row, and a longer identifier that begins with the table.
    assert!(
        !writes_table(
            "sqlx::query(\"update forge_workflow_evidence set qa_passed=true where id=$1\")",
            TABLE
        ),
        "`forge_workflow_evidence` answered for `forge_tool_artifact`"
    );
    assert!(
        writes_table(
            "sqlx::query(\"update forge_workflow_evidence set qa_passed=true where id=$1\")",
            NEIGHBOUR_TABLE
        ),
        "the mirror reading is broken: the neighbour statement no longer answers for its own table"
    );
    assert!(
        !writes_table(
            "sqlx::query(\"update forge_tool_artifact_archive set verdict='PASS' where id=$1\")",
            TABLE
        ),
        "`forge_tool_artifact_archive` answered for `forge_tool_artifact`"
    );

    // The door's own name is not the table's identifier, so it is not a DIRECT spelling…
    assert!(
        !writes_table(
            "sqlx::query_as::<_, ToolArtifactRow>(\"select id from forge_record_tool_artifact($1, $2::uuid, $3, $4, $5, $6, $7, $8)\")",
            TABLE
        ),
        "`forge_record_tool_artifact` was read as a direct spelling of `forge_tool_artifact`"
    );
    // …and detector 2 sees it for what it is: the one door onto the write.
    assert!(
        invokes_door(
            "select id from forge_record_tool_artifact($1, $2::uuid, $3, $4, $5, $6, $7, $8)",
            DOOR
        ),
        "the invocation of `forge_record_tool_artifact` was not seen as the write door"
    );
    assert!(
        !invokes_door("writer.record_tool_artifact(&artifact)", DOOR),
        "the Rust API method name answered for the database function"
    );
    assert!(
        !invokes_door(
            "select qa_passed from forge_workflow_evidence where id=$1",
            DOOR
        ),
        "a file that never names the door was read as a caller of it"
    );

    // Comments are not code: the handbook talks about statements and must not be read as one.
    let stripped =
        code_of_file("/// insert into forge_tool_artifact (story_id) values ($1)\nfn f() {}\n");
    assert!(
        !stripped.contains("insert into") && stripped.contains("fn f"),
        "a comment was read as code, or the line after it was lost: {stripped:?}"
    );
    assert!(
        !writes_table(
            &code_of_file(
                "/// insert into forge_tool_artifact (story_id) values ($1)\nfn f() {}\n"
            ),
            TABLE
        ),
        "a doc comment quoting the statement was counted as a writer"
    );

    // `//` inside a string literal must not truncate the line, and a raw string must survive whole:
    // the SQL in this tree is written in both literal shapes.
    assert!(
        code_of_file("let url = \"https://x.test///\"; let kept = true;\n").contains("kept"),
        "a `//` inside a string ate the rest of the line"
    );
    assert!(
        code_of_file("let q = r#\"insert into forge_tool_artifact (story_id) values ($1)\"#;\nlet quorum = 1;\n")
            .contains("quorum"),
        "a raw string swallowed the rest of the file"
    );

    // The scope filter, in both directions — including THIS file, whose samples quote the statement.
    assert!(
        !is_scanned_path(SELF),
        "this test is inside the scan: its own planted sample would count as a writer"
    );
    assert!(
        !is_scanned_path("tests/src/forge.rs"),
        "the harness fixture client (which writes canonical rows on purpose) is inside the scan"
    );
    assert!(
        !is_scanned_path(GUARD_SOURCE),
        "the production guard's fixtures quote writes and it is inside the scan"
    );
    assert!(
        !is_scanned_path("experiments/pool-bench/src/main.rs"),
        "experiments/ is outside the workspace and inside the scan"
    );
    assert!(
        !is_scanned_path("db/migrations/267_forge_tool_artifact_write.sql"),
        "migrations are not `.rs` and are inside the scan"
    );
    assert!(
        is_scanned_path("db/src/forge_engine.rs") && is_scanned_path("forge/src/roles/smith.rs"),
        "a production file was left outside the scan"
    );

    // ── 2. THE SCAN, against the real tree. ──────────────────────────────────────────────────────────────

    let walked = source::sources_under(&repo_root());
    assert!(
        walked.len() >= RUST_SOURCE_FLOOR,
        "the walk found only {} `.rs` files (floor {RUST_SOURCE_FLOOR}) — a walker that finds nothing \
         reports a clean tree and passes",
        walked.len()
    );
    assert!(
        in_repo(SELF).is_file(),
        "this test does not live where it says it does: {}",
        in_repo(SELF).display()
    );
    // The root must be THIS checkout's: `source::repo_root()` comes from the shared `test_harness` rlib,
    // which a sibling lane may have built (see `repo_root()` above). The scan is immune — it takes the
    // root as an argument — but record the state rather than hide it.
    println!(
        "root under test = {} (source::repo_root() = {})",
        repo_root().display(),
        source::repo_root().display()
    );

    let files = production_files();
    assert!(
        files.len() >= PRODUCTION_RUST_FLOOR,
        "the scan read only {} production `.rs` files (floor {PRODUCTION_RUST_FLOOR}): {files:?}",
        files.len()
    );
    for (path, proof) in READ_PROOF {
        let text = source::read(&in_repo(path));
        assert!(
            text.contains(proof),
            "{path} no longer says `{proof}` — the scan below would pass by reading the wrong file"
        );
    }

    // The two detectors, separately pinned: a direct speller must not hide behind the door, and a second
    // door must not hide behind the empty direct set.
    let direct = direct_writers_of(&files, TABLE);
    assert_eq!(
        direct,
        pinned(&DIRECT_WRITERS),
        "a production file spells `update`/`insert into`/`delete from {TABLE}` directly. The write is \
         the database's (migration 267 `forge_record_tool_artifact`) precisely so the polarity rule is \
         not bypassed; a file that spells it again is a second door. Either route it through the door or \
         name the file in DIRECT_WRITERS with the statement it executes"
    );
    let door_callers = callers_of(&files, DOOR);
    assert_eq!(
        door_callers,
        pinned(&DOOR_CALLERS),
        "the set of production files invoking `{DOOR}` changed. One fact has ONE writer: the door has \
         exactly one caller, and a second one is a second owner of an artifact row"
    );

    let found = writers_of(&files, TABLE, DOOR);
    assert!(
        !found.is_empty(),
        "no writer of {TABLE} was found — an empty scan is a failure, not a pass"
    );
    assert_eq!(
        found,
        pinned(&DOOR_CALLERS),
        "the set of files writing {TABLE} changed. One fact has ONE writer: name the writer here \
         deliberately, or do not join the set"
    );
    assert_eq!(
        found.len(),
        1,
        "forge_tool_artifact must keep exactly one writer — the same single-writer form the guard \
         asserts for forge_workflow_evidence. Found: {}",
        found
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );

    // ── 3. THE BLIND SPOT, bounded: the database's own door and every Rust entry into it. ────────────────

    let migrations = migration_texts();
    assert!(
        migrations.len() >= 100,
        "only {} migrations were read — the migration ledger is part of this contract",
        migrations.len()
    );

    // 3a. Re-derive the writing functions from the migrations themselves: every `create … function`
    // region, run through the SAME direct detector. A new artifact-writing function appears here and
    // fails the pin; a pinned one that stops writing disappears and fails the pin.
    let (sql_functions, one_offs) =
        sql_writer_regions(&migrations, |region| writes_table(region, TABLE));
    let pinned_names: BTreeSet<String> = DB_WRITE_FUNCTIONS
        .iter()
        .map(|(function, _)| function.to_string())
        .collect();
    assert_eq!(
        sql_functions, pinned_names,
        "the set of migration functions whose bodies write {TABLE} changed. One fact has ONE writer: a \
         NEW door owns part of this fact, or a pinned one stopped writing it. Name it here deliberately \
         WITH its production caller, or do not let it exist"
    );
    assert_eq!(
        one_offs,
        pinned(&SQL_ONE_OFFS),
        "the apply-time one-off statements writing {TABLE} changed — a bare statement that writes the \
         table is a second door past the polarity rule and must be named here deliberately"
    );

    // 3b. Each function's production Rust callers, pinned — the door has exactly one.
    for (function, expected) in DB_WRITE_FUNCTIONS {
        assert_eq!(
            callers_of(&files, function),
            pinned(expected),
            "the production callers of `{function}` changed. Every Rust entry into an artifact-writing \
             database function must sit in the pinned set — that is what keeps this scan's blind spot \
             inside the fence"
        );
    }

    // 3c. The subject exists: migration 130 still creates the table the whole contract is about, and it
    // is still a read target of the pinned writer (the DAO's own SELECT) rather than a stranger's.
    assert!(
        migrations.iter().any(|(_, sql)| {
            sql_code(sql)
                .to_ascii_lowercase()
                .contains("create table if not exists forge_tool_artifact")
        }),
        "no migration creates {TABLE} — the table this contract fences has disappeared from the ledger"
    );

    // ── 4. A NEW WRITER FAILS. The planted case: the pin is the fence, and growth is the violation. ──────

    assert!(
        is_scanned_path("db/src/planted_artifact_writer.rs"),
        "a new production writer would not even be scanned — the scope filter is broken"
    );
    let mut grown = found.clone();
    grown.insert("planted/src/new_artifact_writer.rs".to_string());
    assert_ne!(
        grown,
        pinned(&DOOR_CALLERS),
        "a planted second writer was accepted — the pin is not a fence"
    );

    // End to end: a planted file carrying a real write MUST show up in the scan's own answer — through
    // the direct detector (the door detector cannot see it, which is why there are two).
    let mut with_planted = files.clone();
    with_planted.push((
        "db/src/planted_artifact_writer.rs".to_string(),
        code_of_file(
            "sqlx::query(\"insert into forge_tool_artifact (story_id, tool, kind) values ($1, $2, $3)\")",
        ),
    ));
    let planted_found = writers_of(&with_planted, TABLE, DOOR);
    assert!(
        planted_found.contains("db/src/planted_artifact_writer.rs"),
        "the scan would not report a planted unknown writer of {TABLE}"
    );
    assert_eq!(
        direct_writers_of(&with_planted, TABLE),
        pinned(&["db/src/planted_artifact_writer.rs"]),
        "the direct-speller detector would not report a planted unknown writer of {TABLE}"
    );
    assert!(
        writes_table(
            "sqlx::query(\"update forge_tool_artifact set verdict='Failed' where id=$1\")",
            TABLE
        ),
        "the detector that the pin relies on no longer sees a write"
    );

    println!(
        "one-writer/{TABLE} <- {} (direct spellers: {})",
        set_display(&found),
        set_display(&direct)
    );
}

/// The set, printed in its sorted order, for a failure message a reader can act on.
fn set_display(set: &BTreeSet<String>) -> String {
    set.iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

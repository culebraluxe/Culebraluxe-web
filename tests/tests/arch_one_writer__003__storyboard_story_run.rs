//! ARCH.ONE_WRITER — `storyboard_story_run` is written by one pinned set (TST-ARCH-ONE_WRITER-003).
//!
//! CONTRACT. `AGENTS.md`: "Let two sources answer one fact. One fact has ONE writer; if two ever disagree,
//! that is a REFUSAL (HOLD) naming both...". The production guard (`cli/src/forge/repo_guards.rs`,
//! `writer_scan`) freezes the writer SET per audited table; this test is a second, independent reading of
//! the same tree with its own scanner — no import of the guard, no `regex` crate — at the level the guard
//! itself uses, the **TABLE**: which production `.rs` files EXECUTE a write of `storyboard_story_run`
//! (`update` / `insert into` / `delete from` naming the table), as opposed to merely reading it (the
//! `select … from storyboard_story_run` in `forge_read.rs`, `forge_doctor.rs`, `tech.rs`) or naming it in
//! a comment. Where 001 and 002 narrow to a COLUMN, this is the table-level fact the guard pins.
//!
//! THREE FACTS, pinned in **both directions** — a new writer fails here (that is the point) and a pin the
//! tree no longer matches also fails, so the set can only move by a deliberate edit of this file:
//!
//!   1. TWO DAO FILES WRITE THE TABLE, AND NOBODY ELSE INSERTS A RUN. `db/src/forge_control.rs` — one
//!      statement, `interrupt_story_run` (`set ended_at=now(), result_status='Interrupted',
//!      failure_code=$2, notes=…`). `db/src/forge_engine.rs` — four: `stamp_run_base_commit`,
//!      `stamp_run_candidate`, `add_run_usage` and `append_run_detail`. Production Rust has NO `insert
//!      into storyboard_story_run` at all: a run is created by the database (fact 3). Pinned as a set: a
//!      third file joining fails, and so does one leaving — the fence may not move in either direction
//!      quietly.
//!   2. THE TABLE, NOT ITS PREFIX AND NOT ITS SUFFIX (the `\b` problem the guard names). The same scanner
//!      is asked both ways below and must answer only its own name: `update storyboard_story set
//!      status='Ready'…` is NOT a write of the run (the story table), and `update storyboard_story_run
//!      set result_status=…` is NOT a write of the story; `update storyboard_story_run_archive set …` is
//!      neither (identifier boundary after the name); `select … from storyboard_story_run` is a read, not
//!      a write; a `///` comment quoting a statement is not code (comments are stripped first). A scanner
//!      that cannot fail cannot pass, so every direction is planted.
//!   3. THE DATABASE'S OWN DOORS, BOUND AND **DERIVED FROM THE MIGRATIONS**. Two migration functions write
//!      the table: `forge_begin_agent_work_run` (264 — the `insert into storyboard_story_run` that opens a
//!      run when execution begins) and `forge_close_story_run` (263 — the `update … set ended_at,
//!      result_status` that ends one). Two apply-time one-offs backfilled it once when they applied:
//!      `133_story_run_cost_widgets.sql` and `190_forge_run_spend_source.sql`. The set is not taken on
//!      faith — every `create … function` region of `db/migrations` is run through the same detector, so a
//!      NEW run-writing function or one-off fails here instead of slipping past the pin. Rust entry:
//!      `forge_begin_agent_work_run` is invoked from `db/src/forge_engine.rs` and nowhere else, while
//!      `forge_close_story_run` is named by no production `.rs` file at all — it is performed inside SQL by
//!      `forge_finish_agent_work_run` and `forge_reject_agent_work_configuration` (263), and BOTH of those
//!      are invoked from `db/src/forge_engine.rs` and nowhere else. Every Rust entry therefore resolves to
//!      a file already in the pin, and the union built from direct writers, function callers and the
//!      performers' callers is exactly [`WRITERS`].
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * SCOPE. The scan reads `.rs` files under `web/`, `middle/`, `db/`, `cli/`, `forge/` only. `tests/` is
//!     outside it — this file's own planted samples quote `update storyboard_story_run set result_status=…`,
//!     and the exclusion is asserted (not assumed): if the filter broke, the scan would flag this test as a
//!     writer and fail. `cli/src/forge/repo_guards.rs` is excluded for the reason it excludes itself: its
//!     fixtures quote writes (`"UPDATE storyboard_story_run SET result_status='x'"`). `db/migrations/*.sql`
//!     is not `.rs` and `experiments/` is outside the roots, so neither is scanned; `legacy/` is retired
//!     TypeScript and is deliberately not read (Rust only).
//!   * ROOT. The scan's root is resolved inside this file (`env!("CARGO_MANIFEST_DIR")`, the same
//!     expression `test_harness::source::repo_root()` uses) instead of through the helper. The cargo
//!     target dir (`build/rust`) is shared by every lane of this repo and `tests/src/*.rs` is
//!     byte-identical in all of them, so the `test_harness` rlib can be a sibling lane's artifact —
//!     and a sibling's `env!` bakes THAT lane's checkout in (observed 2026-10-04: a run in `lane-mimo`
//!     linked a `lane-nemotron` rlib and would have measured the wrong tree). The helpers that take an
//!     explicit path (`source::sources_under`, `source::read`) are used as they are; only the root is
//!     local, and the SELF check below proves it resolves to this checkout.
//!   * AGREEMENT. The pinned set equals the production guard's `TABLE_WRITERS_BASELINE` row for
//!     `storyboard_story_run` (two files: `db/src/forge_control.rs`, `db/src/forge_engine.rs`) as read on
//!     2026-10-04 — verified by eye, not parsed from the guard: this test's independence is the point, and
//!     if the two ever diverge the divergence is the finding.
//!   * DELETE. The guard's `writes_table` counts `update` and `insert into` only; this reading adds
//!     `delete from`, because a delete writes the fact just as much. The tree has no production `delete
//!     from storyboard_story_run` (nor any migration), so the two readings agree in result — and if one
//!     ever appears, it fails here rather than hiding behind the narrower definition.
//!   * THE AUDIT DOC. `docs/agent/COLUMN-WRITER-AUDIT.md` audits `storyboard_story_run` column by column
//!     (48 rows) and classifies every WRITTEN one with a `legacy/db/*.ts` writer — the retired TypeScript
//!     port. The Rust writers are the two files above; the doc's rows are history, never this scan's
//!     input, and it has no table-level claim to agree or disagree with.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it reads sources, not behaviour.
//! It does not prove the two DAOs move the row *correctly*, does not run a statement, and does not see a
//! write spelled inside a migration function body or fired by a trigger (fact 3 bounds the Rust side of
//! that blind spot — every Rust entry is pinned — and nothing more; SQL-to-SQL calls between migration
//! functions are bounded only at that Rust entry).
//!
//! Level: L0 Pure — filesystem reads only. No database, no network, no process spawned.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__003__storyboard_story_run

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use test_harness::source;

/// The file doing the scanning. Its planted samples quote the statement it hunts, so the scope filter must
/// never count it — asserted below rather than assumed.
const SELF: &str = "tests/tests/arch_one_writer__003__storyboard_story_run.rs";

/// The roots a production writer may live under: the tiers (`web/`, `middle/`, `db/`) and the entry points
/// (`cli/`, `forge/`). `tests/` is deliberately absent — that crate is the fixture client and this suite.
const PRODUCTION_ROOTS: [&str; 5] = ["web/", "middle/", "db/", "cli/", "forge/"];

/// The production guard's source, excluded the way it excludes itself (`is_guard_source`): its test
/// fixtures quote writes, and a scan that counted them would report the guard as a writer of the table.
const GUARD_SOURCE: &str = "cli/src/forge/repo_guards.rs";

/// The table under contract, and the table whose name it is a prefix of — the two must answer only for
/// themselves (fact 2, both directions planted below).
const TABLE: &str = "storyboard_story_run";
const PREFIX_TABLE: &str = "storyboard_story";

/// The production `.rs` files that execute a write of `storyboard_story_run`, frozen. Growth (or a
/// removal) fails until the pin is edited deliberately, which is the act of saying "a third file writes
/// the run ledger".
const WRITERS: [&str; 2] = ["db/src/forge_control.rs", "db/src/forge_engine.rs"];

/// Each pinned writer, and text its RAW source must carry — the scan would otherwise pass by reading the
/// wrong file.
const READ_PROOF: [(&str, &str); 2] = [
    ("db/src/forge_control.rs", "update storyboard_story_run"),
    ("db/src/forge_engine.rs", "update storyboard_story_run"),
];

/// The migration functions whose bodies write the table, and the production `.rs` files allowed to invoke
/// each one from Rust. An empty caller list means the function is SQL-internal: no production Rust file
/// may name it (its own door is [`CLOSE_PERFORMERS`]). The test does not take this list on faith — it
/// re-derives the names from `db/migrations` (every `create … function` region, run through the same
/// detector), so a NEW run-writing function fails here instead of slipping past the pin.
const DB_WRITE_FUNCTIONS: [(&str, &[&str]); 2] = [
    ("forge_begin_agent_work_run", &["db/src/forge_engine.rs"]),
    ("forge_close_story_run", &[]),
];

/// The SQL functions that PERFORM `forge_close_story_run` (263: `forge_finish_agent_work_run` and
/// `forge_reject_agent_work_configuration`) — the only doors onto that write, derived from the migrations
/// the same way. Their production Rust callers are pinned next, which is what keeps the SQL-internal
/// function's blind spot inside the pin instead of open.
const CLOSE_PERFORMERS: [&str; 2] = [
    "forge_finish_agent_work_run",
    "forge_reject_agent_work_configuration",
];

/// The apply-time one-offs: migrations whose nameless region (no `create … function` in the file) writes
/// the table. Each ran ONCE when it applied — a statement with no function to call and no caller to pin.
/// They are writers of the fact, so they are NAMED rather than ignored; a third one (or the removal of
/// either) fails the pin.
const SQL_ONE_OFFS: [&str; 2] = [
    "133_story_run_cost_widgets.sql",
    "190_forge_run_spend_source.sql",
];

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
/// `update storyboard_story` never answers for `update storyboard_story_run` (and vice versa).
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

/// The whole question this test asks of one file's code: does it EXECUTE a write of `table`? Three verbs,
/// because all three move the fact — `update`, `insert into`, and `delete from` (the guard counts the
/// first two; see the DELETE note in the header for why the third is here and why the readings agree).
fn writes_table(code: &str, table: &str) -> bool {
    let normalized_text = normalized(code);
    ["update ", "insert into ", "delete from "]
        .iter()
        .any(|verb| !find_all(&normalized_text, &format!("{verb}{table}")).is_empty())
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

/// The writer set for `table` over an already-read file list.
fn writers_of(files: &[(String, String)], table: &str) -> BTreeSet<String> {
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
/// the apply-time one-offs. A restatement that stops writing drops the name; a new run ledger adds one;
/// both directions fail the pin.
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

/// The migration functions that PERFORM `callee` (a `perform …`/call inside another function's body) —
/// the doors onto an SQL-internal write, found rather than assumed. The callee's own definition region is
/// skipped, or it would name itself.
fn sql_performers_of(migrations: &[(String, String)], callee: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, sql) in migrations {
        for (function, region) in sql_function_regions(sql) {
            if function.is_empty() || function == callee {
                continue;
            }
            if !find_all(&region, callee).is_empty() {
                out.insert(function);
            }
        }
    }
    out
}

#[test]
#[allow(non_snake_case)]
fn arch_one_writer_003__storyboard_story_run() {
    // ── 1. THE DETECTOR, against planted samples. A scan that cannot fail is not a check. ─────────────────

    // A real write of the table is seen — the UPDATE shape (the only one production spells), the INSERT
    // shape (production spells none: creation is the database's door, fact 3) and the DELETE shape.
    assert!(
        writes_table(
            "sqlx::query(\"update storyboard_story_run set result_status='Complete', updated_at=now() where id=$1\")",
            TABLE
        ),
        "a planted `update storyboard_story_run` was not seen"
    );
    assert!(
        writes_table(
            "sqlx::query(\"insert into storyboard_story_run (story_id, started_at) values ($1, now())\")",
            TABLE
        ),
        "a planted `insert into storyboard_story_run` was not seen"
    );
    assert!(
        writes_table(
            "sqlx::query(\"delete from storyboard_story_run where id=$1\")",
            TABLE
        ),
        "a planted `delete from storyboard_story_run` was not seen — a delete writes the fact too"
    );

    // TABLE precision, BOTH directions — the `\b` problem the guard names, asked of both names.
    let run_statement =
        "sqlx::query(\"update storyboard_story_run set result_status='Hold', updated_at=now() where id=$1\")";
    let story_statement = "sqlx::query(\"update storyboard_story set status='Ready' where id=$1\")";
    assert!(
        !writes_table(story_statement, TABLE),
        "`storyboard_story` answered for `storyboard_story_run`"
    );
    assert!(
        !writes_table(run_statement, PREFIX_TABLE),
        "`storyboard_story_run` answered for `storyboard_story` — the `_run` suffix is a different table"
    );
    assert!(
        writes_table(story_statement, PREFIX_TABLE),
        "the mirror reading is broken: the story statement no longer answers for its own table"
    );

    // A longer identifier that BEGINS with the table is a different table.
    assert!(
        !writes_table(
            "sqlx::query(\"update storyboard_story_run_archive set result_status='x' where id=$1\")",
            TABLE
        ),
        "`storyboard_story_run_archive` answered for `storyboard_story_run`"
    );

    // A READ is not a write: the run ledger is read from four production files and written by two.
    assert!(
        !writes_table(
            "sqlx::query_scalar::<_, i64>(\"select count(*) from storyboard_story_run where story_id = $1\")",
            TABLE
        ),
        "a SELECT of the table was read as a write of it"
    );
    assert!(
        !writes_table(
            "sqlx::query(\"update agent_work_item set story_run_id=$2, updated_at=now() where id=$1\")",
            TABLE
        ),
        "an UPDATE of another table was read as a write of the run"
    );

    // Comments are not code: the handbook talks about statements and must not be read as one.
    let stripped = code_of_file(
        "/// update storyboard_story_run set result_status='Hold' where id=$1\nfn f() {}\n",
    );
    assert!(
        !stripped.contains("set result_status") && stripped.contains("fn f"),
        "a comment was read as code, or the line after it was lost: {stripped:?}"
    );
    assert!(
        !writes_table(
            &code_of_file(
                "/// update storyboard_story_run set result_status='Hold' where id=$1\nfn f() {}\n",
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
        code_of_file(
            "let q = r#\"update storyboard_story_run set result_status='x'\"#;\nlet quorum = 1;\n"
        )
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
        !is_scanned_path("db/migrations/264_forge_agent_work_begin.sql"),
        "migrations are not `.rs` and are inside the scan"
    );
    assert!(
        is_scanned_path("db/src/forge_control.rs") && is_scanned_path("db/src/forge_engine.rs"),
        "a production writer was left outside the scan"
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

    let found = writers_of(&files, TABLE);
    assert!(
        !found.is_empty(),
        "no writer of {TABLE} was found — an empty scan is a failure, not a pass"
    );
    assert_eq!(
        found,
        pinned(&WRITERS),
        "the set of files writing {TABLE} changed. One fact has ONE writer: name the writer here \
         deliberately, or do not join the set. A file listed here may only be added with the statement it \
         executes (fact 1 lists each one)"
    );

    // The creation door, separately: production Rust opens no run — `insert into storyboard_story_run`
    // exists only in migration 264 (fact 3). A second INSERT door in `.rs` fails here.
    let inserters: Vec<String> = files
        .iter()
        .filter(|(_, code)| {
            !find_all(&normalized(code), &format!("insert into {TABLE}")).is_empty()
        })
        .map(|(path, _)| path.clone())
        .collect();
    assert!(
        inserters.is_empty(),
        "no production Rust file may INSERT into {TABLE} — a run is opened by \
         `forge_begin_agent_work_run()` (migration 264) at the moment execution begins, with the story's \
         specification snapshotted by that insert itself. Found: {}",
        inserters.join(", ")
    );

    // ── 3. THE BLIND SPOT, bounded: the database's writers and every Rust entry into them. ──────────────

    let migrations = migration_texts();
    assert!(
        migrations.len() >= 100,
        "only {} migrations were read — the migration ledger is part of this contract",
        migrations.len()
    );

    // 3a. Re-derive the function list from the migrations themselves: every `create … function` region,
    // run through the SAME detector. A new run-writing function appears here and fails the pin; a pinned
    // one that stops writing disappears and fails the pin.
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
        "the apply-time one-off statements writing {TABLE} changed — a second migration that writes the \
         ledger outright is a second writer of the fact and must be named here deliberately"
    );

    // 3b. Each function's production Rust callers, pinned — and the union they build.
    let mut union = found.clone();
    for (function, expected) in DB_WRITE_FUNCTIONS {
        let callers = callers_of(&files, function);
        if expected.is_empty() {
            assert!(
                callers.is_empty(),
                "`{function}` is SQL-internal — it is performed by the functions in CLOSE_PERFORMERS, \
                 and no production Rust file may invoke it directly, but these do: {}",
                callers
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        } else {
            assert_eq!(
                callers,
                pinned(expected),
                "the production callers of `{function}` changed. Every Rust entry into a run-writing \
                 database function must sit in the pinned set — that is what keeps this scan's blind \
                 spot inside the fence"
            );
        }
        union.extend(callers);
    }

    // 3c. The SQL-internal door, derived: which functions PERFORM `forge_close_story_run`, and which
    // production file invokes each of them. Both answers must already be inside the pin.
    let performers = sql_performers_of(&migrations, "forge_close_story_run");
    assert_eq!(
        performers,
        pinned(&CLOSE_PERFORMERS),
        "the set of migration functions performing `forge_close_story_run` changed — a new door onto the \
         run's terminal write must be named here with its production caller"
    );
    for performer in CLOSE_PERFORMERS {
        let callers = callers_of(&files, performer);
        assert_eq!(
            callers,
            pinned(&["db/src/forge_engine.rs"]),
            "`{performer}` performs `forge_close_story_run` (263) — its production callers are the only \
             Rust entry onto that write and must stay pinned to the writer already in the set"
        );
        union.extend(callers);
    }

    assert_eq!(
        union,
        pinned(&WRITERS),
        "the set of files that may write {TABLE} — directly, through a run-writing database function, or \
         through the SQL functions that perform one — changed. One fact has ONE writer: name the writer \
         here deliberately, or do not join the set"
    );

    // ── 4. A NEW WRITER FAILS. The planted case: the pin is the fence, and growth is the violation. ──────

    assert!(
        is_scanned_path("db/src/planted_run_writer.rs"),
        "a new production writer would not even be scanned — the scope filter is broken"
    );
    let mut grown = union.clone();
    grown.insert("planted/src/new_run_writer.rs".to_string());
    assert_ne!(
        grown,
        pinned(&WRITERS),
        "a planted third writer was accepted — the pin is not a fence"
    );

    // End to end: a planted file carrying a real write MUST show up in the scan's own answer.
    let mut with_planted = files.clone();
    with_planted.push((
        "db/src/planted_run_writer.rs".to_string(),
        code_of_file(
            "sqlx::query(\"update storyboard_story_run set result_status='Hold', updated_at=now() where id=$1\")",
        ),
    ));
    let planted_found = writers_of(&with_planted, TABLE);
    assert!(
        planted_found.contains("db/src/planted_run_writer.rs"),
        "the scan would not report a planted unknown writer of {TABLE}"
    );
    assert!(
        writes_table(
            "sqlx::query(\"update storyboard_story_run set result_status='Interrupted', updated_at=now() where id=$1\")",
            TABLE
        ),
        "the detector that the pin relies on no longer sees a write"
    );

    println!(
        "one-writer/{TABLE} <- {}",
        union
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );
}

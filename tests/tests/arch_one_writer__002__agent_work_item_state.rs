//! ARCH.ONE_WRITER — `agent_work_item.state` is written by one pinned set (TST-ARCH-ONE_WRITER-002).
//!
//! CONTRACT. `AGENTS.md`: "Let two sources answer one fact. One fact has ONE writer; if two ever disagree,
//! that is a REFUSAL (HOLD) naming both...". The production guard (`cli/src/forge/repo_guards.rs`,
//! `writer_scan`) freezes the writer SET per audited table; this test is a second, independent reading of
//! the same tree with its own scanner — no import of the guard, no `regex` crate — narrowed to the
//! **COLUMN**: which production `.rs` files execute a statement that actually WRITES
//! `agent_work_item.state` (its SET clause or its INSERT column list names the column), as opposed to
//! merely touching the table or merely *reading* the column in a WHERE clause — `set kind=$2 … where
//! state='Ready'` stamps a routing fact on an item that stays Ready, and that is not this contract.
//!
//! THREE FACTS, pinned in **both directions** — a new writer fails here (that is the point) and a pin the
//! tree no longer matches also fails, so the set can only move by a deliberate edit of this file:
//!
//!   1. TWO DAO FILES WRITE THE COLUMN DIRECTLY. `db/src/forge_reset.rs` carries three
//!      `set state='Cancelled'` statements — the story reset, the story recover, and the
//!      stale-open-items sweep — and `db/src/tech.rs` carries one (`withdraw_ready`, the TECH cockpit's
//!      cancellation of a Ready item). The five OTHER production `update agent_work_item` statements —
//!      two in `db/src/forge_control.rs` (flight-member routing and the learn item, setting
//!      `kind`/`model_policy`/`learn_pattern_key`), two more in `db/src/tech.rs` (`stop_after`/
//!      `launch_intent`, then `kind`/`model_policy`) and one in `db/src/forge_engine.rs` (the heartbeat,
//!      `updated_at`) — name `state` only after ` where `, so they are READS of the column. Both
//!      directions are proven against planted samples below, including the real heartbeat shape.
//!   2. THE COLUMN IS THE CONTRACT, IN BOTH DIRECTIONS. `… set state='Cancelled', updated_at=now()
//!      where …` is seen — single-line, multi-line and mixed-case spellings alike; `… set
//!      updated_at=now() where state in ('Claimed','Running')` (the real heartbeat) is NOT; `update
//!      agent_work_item_archive set state=…` is NOT (the identifier boundary after the table);
//!      `set error_text='claimed state=Ready'` is NOT (text after a name is not an assignment); a
//!      `///` comment quoting the statement is NOT (comments are stripped before matching). A scanner
//!      that cannot fail cannot pass, so every direction is planted below.
//!   3. THE DATABASE'S OWN DOORS, BOUND. Ten migration functions write the column:
//!      `agent_work_item_dispatch()` (the creation INSERT — 025, restated 146 and 259),
//!      `forge_claim_specific_agent_work` (262) and `forge_claim_story` (275 — the claim door that obeys the brake; the
//!      262 `forge_claim_next_agent_work` now only delegates to it),
//!      `forge_finish_agent_work_run`/`forge_reject_agent_work_configuration` (263),
//!      `forge_begin_agent_work_run` (264), `forge_reconcile_dispatch_queue` (265), and
//!      `forge_hold_stale_work`/`forge_requeue_stale_work`/`forge_recover_stale_engine_claim` (266).
//!      The set is **derived from the migrations themselves** (each `create … function` region is run
//!      through the same column detector), so a NEW state-writing function fails rather than slipping
//!      past the pin; each name's production Rust callers are then pinned: nine resolve to
//!      `db/src/forge_engine.rs`, `db/src/forge_control.rs` or `db/src/forge_reset.rs`, and
//!      `agent_work_item_dispatch()` resolves to **none** — the trigger `storyboard_story_ready_dispatch`
//!      (025:114) fires it, so creation stays one database door with no Rust caller to pin (and no
//!      production Rust file INSERTs a work item at all — an independent second reading of the guard's
//!      dispatch claim). The trigger is also provoked transitively, and both provocations are pinned:
//!      `db/src/forge_control.rs` flips `storyboard_story.status` into `Ready` directly, and
//!      `forge_dispatch_story()` performs the same flip from SQL with `db/src/forge_engine.rs` as its
//!      only production caller. Taking direct writers, function callers and provokers together, the
//!      union of files that may write the column is exactly the four pinned in [`MAY_WRITE_STATE`].
//!      The apply-time one-off is named rather than ignored: migration 258 flipped `state='Ready'` on
//!      stranded rows once, when it applied — no function, no caller, a writer all the same.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * SCOPE. The scan reads `.rs` files under `web/`, `middle/`, `db/`, `cli/`, `forge/` only. `tests/`
//!     is outside it — this file's own planted samples quote `update agent_work_item set state=…`, and
//!     the exclusion is asserted (not assumed): if the filter broke, the scan would flag this test as a
//!     writer and fail. `cli/src/forge/repo_guards.rs` is excluded for the reason it excludes itself:
//!     its fixtures quote writes (`"insert into agent_work_item (story_id, state, priority) …"`).
//!     `db/migrations/*.sql` is not `.rs` and `experiments/` is outside the roots, so neither is
//!     scanned; `legacy/` is retired TypeScript and is deliberately not read (Rust only).
//!   * ROOT. The scan's root is resolved inside this file (`env!("CARGO_MANIFEST_DIR")`, the same
//!     expression `test_harness::source::repo_root()` uses) instead of through the helper. The cargo
//!     target dir (`build/rust`) is shared by every lane of this repo and `tests/src/*.rs` is
//!     byte-identical in all of them, so the `test_harness` rlib can be a sibling lane's artifact —
//!     and a sibling's `env!` bakes THAT lane's checkout in (observed 2026-10-04: a run in `lane-mimo`
//!     linked a `lane-nemotron` rlib and would have measured the wrong tree). The helpers that take an
//!     explicit path (`source::sources_under`, `source::read`) are used as they are; only the root is
//!     local, and the SELF check below proves it resolves to this checkout.
//!   * AGREEMENT. The production guard has **no** `agent_work_item` row in `TABLE_WRITERS_BASELINE` —
//!     it audits three run/evidence tables, and its `agent_work_item` contract is INSERT-only
//!     (`the_database_owns_dispatch_no_production_rust_file_inserts_a_work_item`: no production Rust
//!     file inserts a work item). This test reproduces that claim independently (fact 3's creation-door
//!     check) and agrees with it, and it adds what the guard does not pin: the UPDATE-side state
//!     writers above. Read 2026-10-04 by eye — this test's independence is the point, and if the two
//!     ever diverge the divergence is the finding, not something to force into agreement.
//!   * THE AUDIT DOC. `docs/agent/COLUMN-WRITER-AUDIT.md` audits `storyboard_story`,
//!     `storyboard_story_run` and `forge_workflow_evidence` — it has no `agent_work_item` section, so
//!     there is no `state` row to agree or disagree with. Its rows are history (retired TypeScript
//!     writers), never this scan's input.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it reads sources, not
//! behaviour. It does not prove the two DAOs transition `state` *correctly*, does not run a statement,
//! and does not see a write spelled inside a migration body or fired by the dispatch trigger (fact 3
//! bounds the Rust side of that blind spot — every Rust entry is pinned, the trigger has none — and
//! nothing more; SQL-to-SQL calls between migration functions are bounded only at that Rust entry).
//!
//! Level: L0 Pure — filesystem reads only. No database, no network, no process spawned.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__002__agent_work_item_state

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use test_harness::source;

/// The file doing the scanning. Its planted samples quote the statement it hunts, so the scope filter must
/// never count it — asserted below rather than assumed.
const SELF: &str = "tests/tests/arch_one_writer__002__agent_work_item_state.rs";

/// The roots a production writer may live under: the tiers (`web/`, `middle/`, `db/`) and the entry points
/// (`cli/`, `forge/`). `tests/` is deliberately absent — that crate is the fixture client and this suite.
const PRODUCTION_ROOTS: [&str; 5] = ["web/", "middle/", "db/", "cli/", "forge/"];

/// The production guard's source, excluded the way it excludes itself (`is_guard_source`): its test
/// fixtures quote writes, and a scan that counted them would report the guard as a writer of the table.
const GUARD_SOURCE: &str = "cli/src/forge/repo_guards.rs";

/// The table and column under contract.
const TABLE: &str = "agent_work_item";
const COLUMN: &str = "state";

/// The files that write `agent_work_item.state` DIRECTLY — an UPDATE whose SET clause names the column
/// (three cancellations in `forge_reset`, one in `tech.rs`) or an INSERT whose column list does (none).
/// Frozen: growth (or a removal) fails until the pin is edited deliberately, which is the act of saying
/// "a third direct state writer exists".
const STATE_WRITERS: [&str; 2] = ["db/src/forge_reset.rs", "db/src/tech.rs"];

/// Every file that MAY write the column: the direct writers above, plus the production Rust callers of
/// the migration functions whose bodies write it (pinned one by one in [`DB_STATE_FUNCTIONS`]), plus the
/// provokers of the dispatch trigger. One fact has ONE writer — this is that writer SET, and it may only
/// move by a deliberate edit of this file.
const MAY_WRITE_STATE: [&str; 4] = [
    "db/src/forge_control.rs",
    "db/src/forge_engine.rs",
    "db/src/forge_reset.rs",
    "db/src/tech.rs",
];

/// The migration functions whose bodies write `agent_work_item.state`, and the production `.rs` files
/// allowed to invoke each one from Rust. An empty caller list is the dispatch trigger's own door: no
/// Rust file may name it. The test does not take this list on faith — it re-derives it from the
/// migrations (every `create … function` region, run through the same column detector), so a NEW
/// state-writing function fails here instead of slipping past the pin.
const DB_STATE_FUNCTIONS: [(&str, &[&str]); 10] = [
    ("agent_work_item_dispatch", &[]),
    (
        "forge_claim_specific_agent_work",
        &["db/src/forge_engine.rs"],
    ),
    // 275: THE claim door. It writes `state` itself (and obeys the brake and the fleet ceiling);
    // `forge_claim_next_agent_work` is now a one-line delegation to it and no longer writes the column, so the
    // writer set did not grow — the claim moved from the old name to this one.
    ("forge_claim_story", &["db/src/forge_engine.rs"]),
    ("forge_begin_agent_work_run", &["db/src/forge_engine.rs"]),
    ("forge_finish_agent_work_run", &["db/src/forge_engine.rs"]),
    (
        "forge_reject_agent_work_configuration",
        &["db/src/forge_engine.rs"],
    ),
    (
        "forge_reconcile_dispatch_queue",
        &["db/src/forge_engine.rs"],
    ),
    ("forge_hold_stale_work", &["db/src/forge_control.rs"]),
    ("forge_requeue_stale_work", &["db/src/forge_control.rs"]),
    (
        "forge_recover_stale_engine_claim",
        &["db/src/forge_reset.rs"],
    ),
];

/// The apply-time one-off: migration 258 flipped `state='Ready'` on rows stranded by an interrupted
/// dispatch — a statement that ran once when the migration applied, with no function to call and no
/// caller to pin. It is a writer of the column, so it is NAMED rather than ignored; a second one-off
/// (or the removal of this one) fails the pin.
const SQL_ONE_OFFS: [&str; 1] = ["258_reopen_stranded_ready_work_items.sql"];

/// The production files that flip `storyboard_story.status` into `Ready` directly. Each flip makes the
/// dispatch trigger's guard (`new.status = 'Ready' and … old.status is distinct from 'Ready'`, 025)
/// INSERT a `Ready` work item — a transitive write of `agent_work_item.state` from a file that never
/// names the table. Provocation is part of "may write", so it lives inside the pin.
///
/// `db/src/tech.rs` joined on 2026-10-04, named deliberately rather than absorbed: the TECH cockpit's own
/// dispatch door (`tech.rs:20`, `update storyboard_story set status='Ready'`) landed with the staged-
/// membership work in `fa1c9f9f`. It is already one of the four frozen writers of `storyboard_story`
/// (`cli/src/forge/repo_guards.rs:165-184`), so it is an approved provoker — but the trigger it fires is
/// this table's transitive write, which is why it belongs in the pin. A third flipper still fails it.
const READY_FLIP_PROVOKERS: [&str; 2] = ["db/src/forge_control.rs", "db/src/tech.rs"];

/// `forge_dispatch_story()` performs the same status flip from SQL (`265:64`), and this is its only
/// production caller — the second provocation door, entering Rust through one pinned file.
const DISPATCH_STORY_PROVOKER: (&str, &[&str]) =
    ("forge_dispatch_story", &["db/src/forge_engine.rs"]);

/// Each pinned file, and text its RAW source must carry — the scan would otherwise pass by reading the
/// wrong file.
const READ_PROOF: [(&str, &str); 4] = [
    ("db/src/forge_reset.rs", "update agent_work_item"),
    (
        "db/src/tech.rs",
        "update agent_work_item set state='Cancelled'",
    ),
    ("db/src/forge_control.rs", "forge_hold_stale_work"),
    ("db/src/forge_engine.rs", "from forge_begin_agent_work_run("),
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

fn is_ident_char(character: u8) -> bool {
    character.is_ascii_alphanumeric() || character == b'_'
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
/// `update agent_work_item` never answers for `update agent_work_item_archive` (and vice versa).
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

/// The statement starting at `start`: up to the first `;` (the Rust statement's end — SQL inside a literal
/// carries none), the next `update ` (a following statement), or a fixed window, whichever comes first.
fn statement_from(normalized_text: &str, start: usize) -> &str {
    const WINDOW: usize = 1200;
    let mut limit = normalized_text.len().min(start + WINDOW);
    while limit > start && !normalized_text.is_char_boundary(limit) {
        limit -= 1;
    }
    let slice = &normalized_text[start..limit];
    let mut cut = slice.len();
    if let Some(offset) = slice.as_bytes()[1..].iter().position(|byte| *byte == b';') {
        cut = cut.min(offset + 1);
    }
    if let Some(offset) = slice[1..].find(" update ") {
        cut = cut.min(offset + 1);
    }
    &slice[..cut]
}

/// The SET clause of a statement: from the first ` set ` to the first ` where ` (or the end).
fn set_clause(statement: &str) -> &str {
    let Some(offset) = statement.find(" set ") else {
        return "";
    };
    let rest = &statement[offset + 5..];
    match rest.find(" where ") {
        Some(at) => &rest[..at],
        None => rest,
    }
}

/// Whether a SET clause assigns the column: `state=`, `set state =`, `, state=` — and never a column
/// merely *read* in a WHERE clause (the clause is cut before ` where `) or a longer name that contains it.
fn assigns_to(clause: &str, column: &str) -> bool {
    let mut from = 0usize;
    while let Some(offset) = clause[from..].find(column) {
        let at = from + offset;
        let before = clause[..at].trim_end();
        let after = clause[at + column.len()..].trim_start();
        let after_set_keyword = before.ends_with("set")
            && (before.len() == 3 || !is_ident_char(before.as_bytes()[before.len() - 4]));
        let at_clause_start = before.is_empty() || before.ends_with(',');
        if (at_clause_start || after_set_keyword) && after.starts_with('=') {
            return true;
        }
        from = at + column.len();
    }
    false
}

/// Whether an `insert into …` statement names the column in its column list — an INSERT that does not name
/// it leaves the column to its DEFAULT and writes nothing.
fn insert_lists_column(statement: &str, column: &str) -> bool {
    let Some(open) = statement.find('(') else {
        return false;
    };
    let Some(close_rel) = statement[open + 1..].find(')') else {
        return false;
    };
    statement[open + 1..open + 1 + close_rel]
        .split(',')
        .map(str::trim)
        .any(|name| name == column)
}

/// The whole question this test asks of one file's code: does it EXECUTE a write of `table.column`?
fn writes_column(code: &str, table: &str, column: &str) -> bool {
    let normalized_text = normalized(code);
    let update = format!("update {table}");
    for start in find_all(&normalized_text, &update) {
        if assigns_to(
            &set_clause(&statement_from(&normalized_text, start)),
            column,
        ) {
            return true;
        }
    }
    let insert = format!("insert into {table}");
    for start in find_all(&normalized_text, &insert) {
        if insert_lists_column(&statement_from(&normalized_text, start), column) {
            return true;
        }
    }
    false
}

/// Whether `code` flips `storyboard_story.status` into `Ready` — the UPDATE that makes the dispatch
/// trigger's guard fire and INSERT a `Ready` work item, i.e. a transitive write of `agent_work_item.state`
/// from a file that never names the table (fact 3's provocation door).
fn sets_status_ready(code: &str) -> bool {
    let normalized_text = normalized(code);
    for start in find_all(&normalized_text, "update storyboard_story") {
        let clause = set_clause(&statement_from(&normalized_text, start));
        if assigns_to(clause, "status") && clause.contains("'ready'") {
            return true;
        }
    }
    false
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

/// The writer set for `table.column` over an already-read file list.
fn writers_of(files: &[(String, String)], table: &str, column: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| writes_column(code, table, column))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The production files whose (comment-stripped) code names `function` — the callers of a database
/// function, found without trusting anyone's list of them.
fn callers_of(files: &[(String, String)], function: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| find_call(code, function))
        .map(|(path, _)| path.clone())
        .collect()
}

/// Whether `code` CALLS `function` — the name present at a word boundary **and applied to arguments**.
///
/// A name merely present in a file does not make its author a caller: `forge/src/pianola/status.rs` holds
/// `"agent_work_item_dispatch"` as one entry of `CANONICAL_THREAT_MARKERS`, a list of strings the Pianola
/// read path warns about, and word-boundary matching alone counted it as a second owner of the creation
/// rule (observed 2026-10-04, once the MAESTRO batch reached `main`). The subject of this pin is invoking
/// the door, so the call shape `name(` is what is matched — the same reading
/// `invokes_door` applies in `arch_one_writer__004__forge_tool_artifact.rs`.
fn find_call(code: &str, function: &str) -> bool {
    find_all(code, function)
        .into_iter()
        .any(|start| code[start + function.len()..].trim_start().starts_with('('))
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
/// functions whose bodies write the fact through `writes`, plus the files whose nameless regions do —
/// the apply-time one-offs. A restatement that stops writing drops the name; a new state machine adds
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
fn arch_one_writer_002__agent_work_item_state() {
    // ── 1. THE DETECTOR, against planted samples. A scan that cannot fail is not a check. ─────────────────

    // A CALL is what the caller pins below are about — a name merely present in a file does not make its
    // author a caller. Both directions are planted: a real application of the door is seen, a quoted marker
    // in a `const …: [&str; …]` list and a bare name are not.
    assert!(
        find_call(
            "async fn run(ctx: &Ctx) { db::dispatch::agent_work_item_dispatch(&ctx).await }",
            "agent_work_item_dispatch"
        ),
        "a planted real call was not seen as a caller"
    );
    assert!(
        !find_call(
            "const CANONICAL_THREAT_MARKERS: [&str; 6] = [\n    \"agent_work_item_dispatch\",\n];",
            "agent_work_item_dispatch"
        ),
        "a quoted marker was read as a caller — a second owner of the creation rule that does not exist"
    );
    assert!(
        !find_call(
            "// agent_work_item_dispatch is the dispatch trigger's own door\nfn f() {}\n",
            "agent_work_item_dispatch"
        ),
        "a name that is never applied to arguments was read as a caller"
    );

    // A real write of the column is seen — the single-line spelling.
    assert!(
        writes_column(
            "sqlx::query(\"update agent_work_item set state='Cancelled', updated_at=now() where id=$1\")",
            TABLE,
            COLUMN
        ),
        "a planted `update … set state=` was not seen"
    );

    // The multi-line spelling: the SQL in this tree is written across lines inside one literal.
    assert!(
        writes_column(
            "sqlx::query(\"update agent_work_item
                set state='Running',
                    updated_at=now()
                  where id=$1::uuid and state in ('Claimed')\")",
            TABLE,
            COLUMN
        ),
        "the multi-line statement spelling was not seen"
    );

    // The mixed-case spelling: `UPDATE … SET State = …` is SQL, and normalization is what sees it.
    assert!(
        writes_column(
            "sqlx::query(\"UPDATE agent_work_item
                SET State = 'Claimed'
              WHERE id = $1::uuid\")",
            TABLE,
            COLUMN
        ),
        "the mixed-case statement spelling was not seen"
    );

    // The INSERT door: a named column is a write, an unnamed column is not (it leaves the column to its
    // DEFAULT). Production has no INSERT here at all — fact 3 checks that separately.
    assert!(
        writes_column(
            "sqlx::query(\"insert into agent_work_item(story_id, state, priority) values ($1, 'Ready', $2)\")",
            TABLE,
            COLUMN
        ),
        "a planted INSERT naming `state` was not seen"
    );
    assert!(
        !writes_column(
            "sqlx::query(\"insert into agent_work_item(story_id, priority) values ($1, $2)\")",
            TABLE,
            COLUMN
        ),
        "an INSERT that never names `state` was read as a writer of it"
    );

    // THE PREDICATE IS NOT THE CONTRACT — the real heartbeat shape: `state` appears, but only in WHERE.
    assert!(
        !writes_column(
            "sqlx::query(\"update agent_work_item set updated_at=now() where id=$1::uuid and state in ('Claimed','Running')\")",
            TABLE,
            COLUMN
        ),
        "the heartbeat (a WHERE-clause read of `state`) was read as a write of it"
    );

    // …and the real routing shape: `kind`/`model_policy` stamped with `state='Ready'` as the guard.
    assert!(
        !writes_column(
            "sqlx::query(\"update agent_work_item set kind=$2, model_policy=$3, updated_at=now() where story_id=$1 and state='Ready'\")",
            TABLE,
            COLUMN
        ),
        "a routing stamp with a WHERE-clause read of `state` was read as a write of it"
    );

    // TABLE precision: a longer identifier that begins with the table is a different table.
    assert!(
        !writes_column(
            "sqlx::query(\"update agent_work_item_archive set state='Ready' where id=$1\")",
            TABLE,
            COLUMN
        ),
        "`agent_work_item_archive` answered for `agent_work_item`"
    );

    // A value that merely CONTAINS the assignment text inside a string is not an assignment of the column.
    assert!(
        !writes_column(
            "sqlx::query(\"update agent_work_item set error_text='claimed state=Ready' where id=$1\")",
            TABLE,
            COLUMN
        ),
        "text inside a SET value was read as an assignment of `state`"
    );

    // Comments are not code: the handbook talks about statements and must not be read as one.
    let stripped = code_of_file(
        "/// update agent_work_item set state='Cancelled', updated_at=now() where id=$1\nfn f() {}\n",
    );
    assert!(
        !stripped.contains("set state") && stripped.contains("fn f"),
        "a comment was read as code, or the line after it was lost: {stripped:?}"
    );
    assert!(
        !writes_column(
            &code_of_file(
                "/// update agent_work_item set state='Cancelled', updated_at=now() where id=$1\nfn f() {}\n",
            ),
            TABLE,
            COLUMN
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
        code_of_file("let q = r#\"update agent_work_item set state='x'\"#;\nlet quorum = 1;\n")
            .contains("quorum"),
        "a raw string swallowed the rest of the file"
    );

    // The provocation detector (fact 3): a flip into `Ready` is seen, a flip into anything else is not,
    // and a WHERE-clause read of `status='Ready'` is not a flip.
    assert!(
        sets_status_ready(
            "sqlx::query(\"update storyboard_story set status='Ready', updated_at=now() where id=$1\")"
        ),
        "a planted flip into `Ready` was not seen"
    );
    assert!(
        !sets_status_ready(
            "sqlx::query(\"update storyboard_story set status='Hold', updated_at=now() where id=$1\")"
        ),
        "a flip into `Hold` was read as a provocation of the dispatch trigger"
    );
    assert!(
        !sets_status_ready(
            "sqlx::query(\"update storyboard_story set rollup=true, updated_at=now() where id=$1 and status='Ready'\")"
        ),
        "a WHERE-clause read of `status='Ready'` was read as a provocation"
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
        !is_scanned_path("db/migrations/258_reopen_stranded_ready_work_items.sql"),
        "migrations are not `.rs` and are inside the scan"
    );
    assert!(
        is_scanned_path("db/src/forge_reset.rs") && is_scanned_path("forge/src/engine/worker.rs"),
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

    let found = writers_of(&files, TABLE, COLUMN);
    assert!(
        !found.is_empty(),
        "no writer of {TABLE}.{COLUMN} was found — an empty scan is a failure, not a pass"
    );
    assert_eq!(
        found,
        pinned(&STATE_WRITERS),
        "the set of files writing {TABLE}.{COLUMN} directly changed. One fact has ONE writer: name the \
         writer here deliberately with the statements it executes (fact 1 lists them), or do not join \
         the set. A file that writes through a migration function instead belongs in MAY_WRITE_STATE \
         via DB_STATE_FUNCTIONS, not here"
    );

    // The creation door, separately: no production Rust file may INSERT a work item at all — the rule the
    // guard carries as `the_database_owns_dispatch…`, reproduced here as an independent second reading.
    let inserters: Vec<String> = files
        .iter()
        .filter(|(_, code)| !find_all(&normalized(code), "insert into agent_work_item").is_empty())
        .map(|(path, _)| path.clone())
        .collect();
    assert!(
        inserters.is_empty(),
        "no production Rust file may INSERT into {TABLE} — creation is `agent_work_item_dispatch()`'s \
         door, written once in the database (025, restated 146 and 259) because a second spelling of an \
         owned rule drifts (migration 146 exists because of exactly that). Found: {}",
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
    // run through the SAME column detector. A new state-writing function appears here and fails the pin;
    // a pinned one that stops writing disappears and fails the pin.
    let (sql_functions, one_offs) =
        sql_writer_regions(&migrations, |region| writes_column(region, TABLE, COLUMN));
    let pinned_names: BTreeSet<String> = DB_STATE_FUNCTIONS
        .iter()
        .map(|(function, _)| function.to_string())
        .collect();
    assert_eq!(
        sql_functions, pinned_names,
        "the set of migration functions whose bodies write {TABLE}.{COLUMN} changed. One fact has ONE \
         writer: a NEW state machine owns part of this fact, or a pinned one stopped writing it. Name it \
         here deliberately WITH its production caller, or do not let it exist"
    );
    assert_eq!(
        one_offs,
        pinned(&SQL_ONE_OFFS),
        "the apply-time one-off statements writing {TABLE}.{COLUMN} changed — a second migration that \
         writes the column outright is a second writer of the fact and must be named here deliberately"
    );

    // 3b. Each function's production Rust callers, pinned — and the union they build.
    let mut union = found.clone();
    for (function, expected) in DB_STATE_FUNCTIONS {
        let callers = callers_of(&files, function);
        if expected.is_empty() {
            assert!(
                callers.is_empty(),
                "`{function}` is the dispatch trigger's own door — no production Rust file may invoke \
                 it, but these do: {} — a second owner of the creation rule",
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
                "the production callers of `{function}` changed. Every Rust entry into a state-writing \
                 database function must sit in the pinned set — that is what keeps this scan's blind \
                 spot inside the fence"
            );
        }
        union.extend(callers);
    }

    // 3c. Creation stays ONE door on the SQL side too: every migration that INSERTs a work item is the
    // dispatch function's own restatement, and the trigger that fires it still exists.
    for (name, sql) in &migrations {
        let text = sql_code(sql);
        if find_all(&normalized(&text), "insert into agent_work_item").is_empty() {
            continue;
        }
        assert!(
            text.contains("function agent_work_item_dispatch("),
            "{name} INSERTs {TABLE} outside `agent_work_item_dispatch()` — creation gained a second door \
             in SQL"
        );
    }
    assert!(
        migrations
            .iter()
            .any(|(_, sql)| sql_code(sql).contains("execute function agent_work_item_dispatch")),
        "the dispatch trigger `storyboard_story_ready_dispatch` is gone from db/migrations — creation has \
         no door at all, which is not the contract either"
    );

    // 3d. Provocation: the trigger's INSERT can only be reached from files inside the pin. A direct flip
    // into `Ready`, the SQL flip performed by `forge_dispatch_story()`, and the story INSERT door all
    // fire the same guard — every one of them must already be a file this pin names.
    let flippers: BTreeSet<String> = files
        .iter()
        .filter(|(_, code)| sets_status_ready(code))
        .map(|(path, _)| path.clone())
        .collect();
    assert_eq!(
        flippers,
        pinned(&READY_FLIP_PROVOKERS),
        "the set of files flipping storyboard_story.status into `Ready` changed — each flip makes the \
         dispatch trigger INSERT a work item, i.e. write {TABLE}.{COLUMN} through a file that never \
         names the table"
    );
    let (provoker_function, provoker_expected) = DISPATCH_STORY_PROVOKER;
    let provoker_callers = callers_of(&files, provoker_function);
    assert_eq!(
        provoker_callers,
        pinned(provoker_expected),
        "`{provoker_function}` performs the same flip from SQL (265) — its production callers are the \
         provocation's Rust entry and must stay pinned"
    );
    union.extend(flippers);
    union.extend(provoker_callers);

    let story_inserters: BTreeSet<String> = files
        .iter()
        .filter(|(_, code)| !find_all(&normalized(code), "insert into storyboard_story").is_empty())
        .map(|(path, _)| path.clone())
        .collect();
    assert!(
        story_inserters.is_subset(&union),
        "a production file INSERTs a story — a row born `Ready` fires the dispatch trigger — and it sits \
         outside the pinned union: {}",
        story_inserters
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );

    assert_eq!(
        union,
        pinned(&MAY_WRITE_STATE),
        "the set of files that may write {TABLE}.{COLUMN} — directly, through a state-writing database \
         function, or by provoking the dispatch trigger — changed. One fact has ONE writer: name the \
         writer here deliberately, or do not join the set"
    );

    // ── 4. A NEW WRITER FAILS. The planted case: the pin is the fence, and growth is the violation. ──────

    assert!(
        is_scanned_path("db/src/planted_state_writer.rs"),
        "a new production writer would not even be scanned — the scope filter is broken"
    );
    let mut grown = union.clone();
    grown.insert("planted/src/new_state_writer.rs".to_string());
    assert_ne!(
        grown,
        pinned(&MAY_WRITE_STATE),
        "a planted new writer was accepted — the pin is not a fence"
    );

    // End to end: a planted file carrying a real write MUST show up in the scan's own answer.
    let mut with_planted = files.clone();
    with_planted.push((
        "db/src/planted_state_writer.rs".to_string(),
        code_of_file(
            "sqlx::query(\"update agent_work_item set state='Ready', updated_at=now() where id=$1\")",
        ),
    ));
    let planted_found = writers_of(&with_planted, TABLE, COLUMN);
    assert!(
        planted_found.contains("db/src/planted_state_writer.rs"),
        "the scan would not report a planted unknown writer of {TABLE}.{COLUMN}"
    );
    assert!(
        writes_column(
            "sqlx::query(\"update agent_work_item set state='Running', updated_at=now() where id=$1\")",
            TABLE,
            COLUMN
        ),
        "the detector that the pin relies on no longer sees a write"
    );

    println!(
        "one-writer/{TABLE}.{COLUMN} <- {}",
        union
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );
}

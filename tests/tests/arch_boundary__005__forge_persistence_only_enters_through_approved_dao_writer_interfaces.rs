//! ARCH.BOUNDARY — Forge persistence enters only through approved DAO and writer interfaces (TST-ARCH-BOUNDARY-005).
//!
//! Contract: `forge` decides, records and publishes. It does not own a connection and it holds no statement.
//! Across its 111 source files there is no `sqlx`, no pool, no driver URL and no SQL in code; persistence enters
//! through the `db` crate's DAO and writer functions, which 24 of those files name. That is the difference between a
//! run whose state is queryable and auditable — the same rows every other lane reads — and a run that keeps its own
//! table because it could.
//!
//! The positive control is part of the contract, not decoration: a Forge crate that named no DAO seam at all would be
//! clean for the wrong reason, so the same scan that refuses SQL also has to find the seam in use.
//!
//! **No exception, and that is recent (`2026-10-02`, the seam closure).** `engine/observer.rs` used to write its own
//! `INSERT INTO workflow_execution_trace_event` and was pinned here as a known exception. The statement is the flight
//! recorder's (`FlightRecorderDao::TRACE_EVENT_INSERT_SQL`, reached through `ForgeEngineDao::record_observer`), the
//! copy in Forge was dead, and it is gone — so this test now asserts an *empty* finding list. A second file acquiring
//! SQL fails here, and so would the first one coming back.
//!
//! One statement, one owner — and the check therefore leaves Forge's crate. That table has two writers: the workflow
//! kernel (as `workflow`/`workflow_engine`) and Forge's observer (as `forge_observer`). Each used to spell the
//! `INSERT` itself, casts and `on conflict` predicate included, so the replay backstop migration 090 installs as a
//! partial unique index had two Rust copies to keep in step with it. The kernel binds the recorder's statement now,
//! and the scan below refuses a second holder anywhere outside a test.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__005__forge_persistence_only_enters_through_approved_dao_writer_interfaces

//! **The scan reads statements, not lines (`2026-10-04`).** `production_lines` drops `#[cfg(test)]` items, joins
//! `\`-continued lines and strips a comment only where a `//` really starts one; `statement_shaped` requires a SQL
//! phrase to be followed by what it acts on. Both halves were needed. The guard had three false positives in
//! `forge/src/pianola/status.rs` — a threat-marker vocabulary and two test fixtures — and it had *missed* a real
//! `select … from forge_tool_artifact … in (…)` in that same file, because the statement was wrapped across
//! continuations, and because a `split("//")` reads the `//` of a `postgres://` URL as a comment. The controls at
//! the foot of this file hold both halves: a vocabulary stays quiet, and a wrapped statement is still a statement.

use test_harness::source;

/// What a Forge module may never contain: a database client, a driver URL, or a SQL statement in code.
///
/// `code` is one logical line from [`production_lines`] — already comment-and-test stripped, continuations joined —
/// so a phrase matched here is a phrase in the code, and a phrase followed by what it acts on is a statement.
fn direct_db_access(line: &str) -> Option<&'static str> {
    let code = code_before_comment(line);
    for client in [
        "sqlx",
        "PgPool",
        "PgConnection",
        "postgres://",
        "postgresql://",
    ] {
        if code.contains(client) {
            return Some("a database client in Forge");
        }
    }
    let code = code.to_lowercase();
    for statement in [
        "insert into",
        "delete from",
        "alter table",
        "create table",
        "drop table",
        "on conflict",
    ] {
        if statement_shaped(&code, statement) {
            return Some("a SQL statement in Forge");
        }
    }
    if source::contains_word(&code, "update") && source::contains_word(&code, "set") {
        return Some("an UPDATE in Forge");
    }
    if source::contains_word(&code, "select") && source::contains_word(&code, "from") {
        return Some("a SELECT in Forge");
    }
    None
}

/// The canonical trace `INSERT`, lowercased: the check is about the statement, not about its casing.
const TRACE_INSERT: &str = "insert into workflow_execution_trace_event";

// ---------------------------------------------------------------------------
// The statement view: why this is a check about statements and not about lines
// ---------------------------------------------------------------------------

/// The code part of a line: everything before a `//` **that is not inside a string literal**.
///
/// The local, precise form of [`source::code_of`] for this guard, and the difference is not cosmetic: the shared
/// helper cuts at the first `//` anywhere, so `let url = "postgres://prod"` reads as `let url = "postgres:` and a
/// driver URL held as a value is invisible to the check whose whole job is to refuse it. A character literal that
/// holds a quote (`'"'`) is left alone; where this is wrong it is wrong by *missing* text, never by inventing a
/// statement — and a missing statement is what the floor assertion below is for.
fn code_before_comment(line: &str) -> String {
    let mut out = String::new();
    let mut in_string = false;
    let mut previous = '\0';
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_string => {
                out.push(c);
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            }
            '"' if previous != '\'' => {
                in_string = !in_string;
                out.push(c);
            }
            '/' if !in_string && chars.peek() == Some(&'/') => break,
            _ => out.push(c),
        }
        previous = c;
    }
    out
}

/// One file's production code as logical lines — `(1-based line number, code)` — with `#[cfg(test)]` items
/// removed, comments stripped, and `\`-continued lines joined onto the line they continue.
///
/// Three holes, and the guard has produced a miss or a false positive through every one of them:
///   * a `#[cfg(test)]` module is not production code, and its fixtures quote statements on purpose — in this
///     guard's own subject `forge/src/pianola/status.rs` a summary string and an injection-guard input both read as
///     violations, and `engine/opencode.rs` holds a `postgres://prod` test fixture;
///   * a statement wrapped across `\` continuations is **one** statement, so reading line by line turned the
///     `select … from forge_tool_artifact … in (…)` that `status.rs` held into three harmless fragments;
///   * a comment is not code — but only a `//` outside a string literal starts one.
fn production_lines(text: &str) -> Vec<(usize, String)> {
    let raw: Vec<&str> = text.lines().collect();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        let code = code_before_comment(raw[index]);
        if code.trim() == "#[cfg(test)]" {
            let mut probe = index + 1;
            while probe < raw.len()
                && (raw[probe].trim().is_empty() || raw[probe].trim().starts_with("#["))
            {
                probe += 1;
            }
            index = if probe < raw.len() && raw[probe].contains('{') {
                // An item with a body ends where a top-level item ends: the first `}` at column 0.
                let mut end = probe;
                while end < raw.len() && !raw[end].starts_with('}') {
                    end += 1;
                }
                end + 1
            } else {
                // The attribute guards one line and nothing after it (`use`, `const`, a bodiless `fn`).
                probe
            };
            continue;
        }
        match out.last_mut() {
            Some((_, last)) if last.trim_end().ends_with('\\') => {
                last.push(' ');
                last.push_str(code.trim());
            }
            _ => out.push((index + 1, code)),
        }
        index += 1;
    }
    out
}

/// A statement names what it acts on; a vocabulary does not.
///
/// `"create table"` by itself is a marker word — Forge's threat vocabulary (`CANONICAL_THREAT_MARKERS`) is a list
/// of them, and that list is not SQL — while a statement reads `create table pianola_queue`, `delete from
/// "quoted"`, or `on conflict (source_system, source_event_id)`. So the phrase must be followed by an identifier
/// (bare or quoted) or by `(`.
fn statement_shaped(code: &str, phrase: &str) -> bool {
    let lower = code.to_lowercase();
    let mut search_from = 0;
    while let Some(at) = lower[search_from..].find(phrase) {
        let start = search_from + at;
        let mut rest = lower[start + phrase.len()..].trim_start().chars();
        match rest.next() {
            Some(c) if c.is_alphanumeric() || c == '_' => return true,
            Some('"') if rest.next().is_some_and(|c| c.is_alphanumeric() || c == '_') => {
                return true
            }
            Some('(') => return true,
            _ => {}
        }
        search_from = start + phrase.len();
    }
    false
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-005); the file and the assay use it.
fn arch_boundary_005__forge_persistence_only_enters_through_approved_dao_writer_interfaces() {
    let forge_src = source::workspace_root().join("forge/src");
    let files = source::sources_under(&forge_src);
    assert!(
        files.len() >= 80,
        "the Forge crate is the subject of this contract; only {} files were found under {}",
        files.len(),
        source::relative(&forge_src)
    );

    let mut findings = Vec::new();
    let mut via_dao = 0usize;
    let mut logical_lines = 0usize;
    for path in &files {
        let text = source::read(path);
        if text.contains("db::") {
            via_dao += 1;
        }
        let view = production_lines(&text);
        logical_lines += view.len();
        for (number, code) in view {
            if let Some(what) = direct_db_access(&code) {
                findings.push(format!(
                    "{}:{}: {what}: {}",
                    source::relative(path),
                    number,
                    code.trim()
                ));
            }
        }
    }

    // The view has to have looked at the crate: a stripped-to-nothing view would report a clean Forge for the wrong
    // reason. The floor is loose on purpose (the statement is the findings list, not this number) and the planted
    // sample at the foot of this file proves the view keeps production lines on both sides of a `#[cfg(test)]` item.
    println!(
        "statement view: {logical_lines} logical lines over {} files",
        files.len()
    );
    assert!(
        logical_lines >= 20_000,
        "the statement view found only {logical_lines} logical lines in {} — it is reading almost nothing",
        source::relative(&forge_src)
    );

    // No exception and no allowance: every statement in Forge `src/` is a finding, so this list has to be empty. The
    // observer's `INSERT` lives in the DAO (`ForgeEngineDao::record_observer`) and its dead copy here is gone.
    let drivers: Vec<&String> = findings
        .iter()
        .filter(|finding| finding.contains("database client in Forge"))
        .collect();
    let statements: Vec<&String> = findings
        .iter()
        .filter(|finding| !finding.contains("database client in Forge"))
        .collect();
    assert!(
        drivers.is_empty(),
        "Forge owns no connection and links no driver:\n{}",
        drivers
            .iter()
            .map(|finding| finding.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let mut sql_files: Vec<String> = statements
        .iter()
        .map(|finding| finding.split(':').next().unwrap_or("").to_string())
        .collect();
    sql_files.sort();
    sql_files.dedup();
    assert!(
        sql_files.is_empty(),
        "Forge holds no SQL of its own; every write goes through a db DAO or writer:\n{}",
        statements
            .iter()
            .map(|finding| finding.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert!(
        via_dao >= 10,
        "Forge must reach persistence through the `db::` DAO seam (only {via_dao} files name it, so this is not the crate under test)"
    );

    // The statement did not vanish with the copy, so the authority is asserted here: "no SQL in Forge" must never be
    // able to mean "the trace row has no writer". The flight recorder holds the one spelling — two systems write this
    // table, and the dedupe both depend on is `on conflict (source_system, source_event_id)` against migration 090's
    // partial unique index, so a second spelling of that predicate is a second thing to keep in step with it.
    let owner_path = source::workspace_root().join("db/src/flight_recorder/sibling_limit.rs");
    let owner = source::read(&owner_path);
    assert!(
        owner.contains("pub const TRACE_EVENT_INSERT_SQL")
            && owner.contains("on conflict (source_system, source_event_id)"),
        "the flight recorder holds the one spelling of the trace `INSERT`, replay backstop included"
    );
    let dao = source::read(&source::workspace_root().join("db/src/forge_engine.rs"));
    assert!(
        dao.contains("TRACE_EVENT_INSERT_SQL") && dao.contains("pub async fn record_observer"),
        "`ForgeEngineDao::record_observer` binds the recorder's statement rather than holding a copy of it"
    );
    let caller = source::read(&forge_src.join("engine/observer.rs"));
    assert!(
        caller.contains("dao.record_observer("),
        "Forge must still write the trace row — through the DAO, never with SQL of its own"
    );
    let kernel = source::read(&source::workspace_root().join("middle/workflow/src/neon/new_id.rs"));
    assert!(
        kernel.contains("TRACE_EVENT_INSERT_SQL"),
        "the workflow kernel is the second writer at this table, so its `insert_event` binds the same statement — the \
         third spelling of it is what this line keeps dead"
    );

    // And the scan above is what makes that true rather than intended: the statement's text survives in exactly one
    // non-test file of the workspace, the owner named above.
    let workspace = source::sources_under(&source::workspace_root());
    assert!(
        workspace.len() >= 300,
        "the workspace scan is half the subject; only {} Rust files were found under {}",
        workspace.len(),
        source::relative(&source::workspace_root())
    );
    let holders: Vec<String> = workspace
        .iter()
        // A test may quote a statement — this file does, above. Production code may not hold a second copy of one.
        .filter(|path| !source::relative(path).contains("/tests/"))
        .filter(|path| {
            source::read(path)
                .lines()
                .any(|line| source::code_of(line).to_lowercase().contains(TRACE_INSERT))
        })
        .map(|path| source::relative(path))
        .collect();
    assert_eq!(
        holders,
        vec![source::relative(&owner_path)],
        "one file holds this statement and its two writers bind that one; a second holder is a copy to keep in step"
    );

    // The negative control: the detector fires on a query, and stays quiet on Forge's own vocabulary and on prose —
    // including the trait method `.execute(&envelope)` that exists in the tree (`forge/src/engine/git_publish.rs:128`)
    // and is a command dispatch, not a statement.
    assert!(direct_db_access("let rows = sqlx::query_as::<_, Row>(\"select id from storyboard_story\").fetch_all(pool).await?;").is_some());
    assert!(direct_db_access("let pool = PgPool::connect(&database_url).await?;").is_some());
    assert_eq!(
        direct_db_access("let out = envelope.execute(&name, &request.input);"),
        None,
        "an envelope's execute is not a SQL statement"
    );
    let prose = production_lines("// the DAO selects from the work item table for us");
    assert!(
        prose
            .iter()
            .all(|(_, line)| direct_db_access(line).is_none()),
        "prose about a DAO's own query is not a query here"
    );

    // The precision and recall controls for the statement view — each one a defect this guard really had, in the
    // file it was raised about, so a regression is caught here rather than by a lane's next T1 run.
    //
    // (a) A marker vocabulary is not a statement. `forge/src/pianola/status.rs` holds `"create table"` inside
    //     `CANONICAL_THREAT_MARKERS` — a list of words to *look for* in a model's summary — and that was reported
    //     as SQL in Forge.
    assert_eq!(
        direct_db_access("    \"create table\","),
        None,
        "a threat-marker word is not a statement"
    );
    assert_eq!(
        direct_db_access("    \"insert into\","),
        None,
        "a threat-marker word is not a statement"
    );
    assert!(
        direct_db_access("let sql = \"create table pianola_queue (id text)\";").is_some(),
        "the same phrase naming what it acts on is a statement, and a statement in Forge is a finding"
    );

    // (b) A `#[cfg(test)]` item quotes statements on purpose — `status.rs` has two such fixtures and
    //     `engine/opencode.rs` holds `postgres://prod` — so the view drops the item *and keeps the rest of the
    //     file*, which is the half that could quietly hide a real statement.
    let sample = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let fixture = \"create table fixture_queue (id text)\";\n    }\n}\nfn production() {\n    let sql = \"create table pianola_queue (id text)\";\n}\n";
    let view = production_lines(sample);
    assert!(
        view.iter().all(|(_, line)| !line.contains("fixture_queue")),
        "a `#[cfg(test)]` item leaves the statement view: {view:?}"
    );
    assert!(
        view.iter()
            .any(|(_, line)| direct_db_access(line).is_some()),
        "production code after a `#[cfg(test)]` item is still read: {view:?}"
    );

    // (c) A statement wrapped across `\` continuations is one statement. This is the shape `status.rs` held
    //     (`select … from forge_tool_artifact where story_id in (…)`) while this guard, reading line by line,
    //     reported nothing at all.
    let wrapped = "let sql = format!(\"select id::text \\\n     from forge_tool_artifact where story_id = any($1::text)\");";
    assert!(
        wrapped.lines().all(|line| direct_db_access(line).is_none()),
        "line by line this statement is invisible — which is how it was missed"
    );
    assert!(
        production_lines(wrapped)
            .iter()
            .any(|(_, line)| direct_db_access(line).is_some()),
        "joined across its continuation it is one statement, and a statement in Forge is a finding"
    );

    // (d) A driver URL inside a string literal is not a comment: `split(\"//\")` read the `//` of `postgres://` as
    //     the start of one, so a URL held as a value was invisible to the check that exists to refuse it.
    assert_eq!(
        direct_db_access("let url = \"postgres://user@host/db\";"),
        Some("a database client in Forge"),
        "a driver URL in a string literal is a driver URL"
    );
}

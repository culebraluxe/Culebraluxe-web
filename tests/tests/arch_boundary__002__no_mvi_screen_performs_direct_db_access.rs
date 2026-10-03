//! ARCH.BOUNDARY — no MVI screen performs direct DB access (TST-ARCH-BOUNDARY-002).
//!
//! Contract: a screen is a reducer and a view. It decides state and renders it; it never opens a connection. The only
//! data path out of a screen is the URL it calls (`web/ui/src/app/api.rs`) or the command it dispatches to the
//! service boundary — which is why `web/ui/Cargo.toml:13-56` lists `domain`, `serde`, `chrono` and the wasm bindings
//! and no database crate at all, and why all 100 files under `web/ui/src/app/screens/**` are free of SQL today.
//!
//! The rule is proven against the sources themselves, so it cannot drift: if a screen ever grows a query, the tree
//! fails this test rather than the browser failing in production. The detector is deliberately word-accurate — it
//! fires on `sqlx`, on a driver URL, and on a real statement whose `select` and `from` are whole words — so
//! `selected`, `from_gallery` and a comment about "selecting from the gallery" are not evidence of anything.
//!
//! Non-vacuous by construction: the walker must find the screens, and the detector must flag planted samples that are
//! never on disk. A scanner that inspected no files (wrong path) would otherwise report a clean tree.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__002__no_mvi_screen_performs_direct_db_access

/// What a screen may never contain: a database client, a driver URL, or a SQL statement in code.
///
/// The line's comment is dropped before the SQL check, so prose about a screen that "selects from" a list is not a
/// finding, and a whole-word match keeps `selected` and `from_gallery` out of the verdict.
fn direct_db_access(line: &str) -> Option<&'static str> {
    const CLIENTS: [&str; 4] = ["sqlx", "postgres://", "postgresql://", "PgPool"];
    for client in CLIENTS {
        if line.contains(client) {
            return Some("a database client in a screen");
        }
    }
    let code = line.split("//").next().unwrap_or("").to_lowercase();
    const STATEMENTS: [&str; 6] = [
        "insert into",
        "delete from",
        "alter table",
        "create table",
        "drop table",
        "on conflict",
    ];
    for statement in STATEMENTS {
        if code.contains(statement) {
            return Some("a SQL statement in a screen");
        }
    }
    // An UPDATE is only a statement when it sets something: `ProjectsCalendarUpdate { .. }` is a message, not a query.
    if contains_word(&code, "update") && contains_word(&code, "set") {
        return Some("an UPDATE in a screen");
    }
    if contains_word(&code, "select") && contains_word(&code, "from") {
        return Some("a SELECT in a screen");
    }
    None
}

/// Whole-word containment, so `selected` is not `select`.
fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let before = haystack[..start]
            .chars()
            .next_back()
            .map(|c| !c.is_alphanumeric() && c != '_')
            .unwrap_or(true);
        let after = haystack[start + needle.len()..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric() && c != '_')
            .unwrap_or(true);
        before && after
    })
}

/// The repository root — every Rust crate is a tier directory (`web/`, `middle/`, `db/`) or an entry point
/// (`cli/`, `forge/`) under it; this suite lives in `tests/`, one level below it.
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the suite lives in tests/, one level below the repository root")
        .to_path_buf()
}

/// Every `.rs` file under `dir`, skipping build output.
fn rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), "target" | "node_modules" | ".git") {
                continue;
            }
            rust_sources(&path, out);
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-002); the file and the assay use it.
fn arch_boundary_002__no_mvi_screen_performs_direct_db_access() {
    let rust = workspace_root();
    let screens = rust.join("web/ui/src/app/screens");
    assert!(
        screens.is_dir(),
        "the MVI screens live at web/ui/src/app/screens; the scan cannot be vacuous"
    );

    let mut paths = Vec::new();
    rust_sources(&screens, &mut paths);
    assert!(
        paths.len() >= 50,
        "the screen tree is the subject of this contract; only {} files were found under {}",
        paths.len(),
        screens.display()
    );

    let mut offenders = Vec::new();
    for path in &paths {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        for (number, line) in text.lines().enumerate() {
            if let Some(what) = direct_db_access(line) {
                offenders.push(format!(
                    "{}:{}: {what}: {}",
                    path.strip_prefix(&rust).unwrap_or(path).display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a screen must not reach the database; it renders state or calls the API:\n{}",
        offenders.join("\n")
    );

    // The negative control. The detector is proven to fire on what a screen must never contain, and to stay quiet on
    // the prose and the identifiers that merely look like SQL — so a clean tree is evidence, not silence.
    assert!(
        direct_db_access(
            "let rows = query(\"select id from property_media\").fetch_all(pool).await;"
        )
        .is_some(),
        "the detector must catch a SELECT that is not prefixed by a client name"
    );
    assert!(
        direct_db_access("let mut client = sqlx::PgPool::connect(&url).await?;").is_some(),
        "the detector must catch a database client"
    );
    assert!(
        direct_db_access("// the gallery selects from the property's media list").is_none(),
        "prose about selecting is not a query"
    );
    assert!(
        direct_db_access("let selected = rows.iter().filter(|r| r.from_gallery).count();")
            .is_none(),
        "`selected` and `from_gallery` are not SQL"
    );
}

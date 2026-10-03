//! ARCH.BOUNDARY — the domain cannot depend on server, UI or integrations (TST-ARCH-BOUNDARY-004).
//!
//! Contract: `middle/model` is the innermost layer. It knows business nouns and rules, and it knows nothing about
//! who calls it: not the HTTP server, not the browser bundle, not a provider integration, not a database driver and
//! not a runtime. Its dependency table is the whole proof of that — `middle/model/Cargo.toml:7-13` lists six
//! external crates (`chrono`, `serde`, `serde_json`, `sha2`, `thiserror`, `unicode-normalization`), every one of them
//! a pure library, and **no path dependency at all**, which is the only way a sibling workspace crate could reach it.
//! The 56 files under `middle/model/src` agree: not one of them names `server`, `ui`, `integrations`, `service`,
//! `forge` or `db`.
//!
//! The direction is what matters. `server` depends on `domain`, and `domain` depending back on `server` — directly or
//! through the driver it would then have to link — is how a domain rule ends up needing an HTTP request to run, a bug
//! this port has already paid for once. The rule is checked as a table (keys, not text) plus a source scan, and both
//! detectors are proven against forged input, so neither half can pass by finding nothing.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__004__domain_cannot_depend_on_server_ui_integrations

use test_harness::source;

/// Crates that may not be reachable from the domain: workspace members that own I/O, and the I/O stacks themselves.
const FORBIDDEN: [&str; 22] = [
    "db",
    "web",
    "ui",
    "apis",
    "services",
    "forge",
    "cli",
    "workflow",
    "auth",
    "test-harness",
    "tokio",
    "axum",
    "sqlx",
    "reqwest",
    "hyper",
    "tower",
    "yew",
    "gloo-net",
    "wasm-bindgen",
    "js-sys",
    "web-sys",
    "tauri",
];

/// A dependency table entry that is a path dependency, i.e. a sibling crate in this workspace.
fn is_path_dependency(key: &str, manifest: &str) -> bool {
    let key = source::crate_name_of(key);
    manifest.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&key) && trimmed.contains("path =")
    })
}

/// A foreign crate named in a domain source file's `use` statement.
fn foreign_crate_in(line: &str) -> Option<String> {
    let trimmed = source::code_of(line).trim_start();
    let rest = trimmed.strip_prefix("use ")?.trim();
    let root = rest.split("::").next()?.split([';', ' ']).next()?.trim();
    // `crate`, `self`, `super`, `std`, `core` and `alloc` are the language's own paths; anything else is a crate.
    if matches!(
        root,
        "" | "crate" | "self" | "super" | "std" | "core" | "alloc"
    ) {
        None
    } else {
        Some(root.to_string())
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-004); the file and the assay use it.
fn arch_boundary_004__domain_cannot_depend_on_server_ui_integrations() {
    // 1. The manifest. `serde` must be present, so a parse that found nothing cannot pass as a clean table.
    let manifest = source::read(&source::rust_root().join("middle/model/Cargo.toml"));
    let declared: Vec<String> = source::manifest_keys(&manifest, "dependencies")
        .iter()
        .map(|key| source::crate_name_of(key))
        .collect();
    assert!(
        declared.iter().any(|crate_name| crate_name == "serde"),
        "the domain's dependency table must have been found; it lists serde (found: {declared:?})"
    );

    let linked: Vec<String> = declared
        .iter()
        .filter(|crate_name| FORBIDDEN.contains(&crate_name.as_str()))
        .cloned()
        .collect();
    assert!(
        linked.is_empty(),
        "the domain is the innermost layer and cannot link these: {linked:?}"
    );

    // A path dependency is how a workspace sibling enters without naming itself in a way the list above could miss.
    let path_entries: Vec<&String> = declared
        .iter()
        .filter(|crate_name| is_path_dependency(crate_name, &manifest))
        .collect();
    assert!(
        path_entries.is_empty(),
        "the domain has no path dependencies; a sibling crate here would break the direction: {path_entries:?}"
    );

    // 2. The sources. A `use` of any crate other than the language's own is the same violation in code.
    let domain_src = source::rust_root().join("middle/model/src");
    let files = source::sources_under(&domain_src);
    assert!(
        files.len() >= 40,
        "the domain source tree is the subject of this contract; only {} files were found under {}",
        files.len(),
        source::relative(&domain_src)
    );

    let mut findings = Vec::new();
    for path in &files {
        for (number, line) in source::read(path).lines().enumerate() {
            if let Some(crate_name) = foreign_crate_in(line) {
                if FORBIDDEN.contains(&crate_name.as_str()) {
                    findings.push(format!(
                        "{}:{}: `use {crate_name}`",
                        source::relative(path),
                        number + 1
                    ));
                }
            }
        }
    }
    assert!(
        findings.is_empty(),
        "the domain cannot reach outward:\n{}",
        findings.join("\n")
    );

    // 3. The negative control. Every detector above must fire on the violation it exists to catch.
    assert_eq!(
        foreign_crate_in("use web::api::routes;"),
        Some("web".to_string()),
        "a `use web::` in the domain is a finding"
    );
    assert_eq!(
        foreign_crate_in("use sqlx::PgPool;"),
        Some("sqlx".to_string())
    );
    assert_eq!(
        foreign_crate_in("use crate::property::Property;"),
        None,
        "the domain's own modules are not foreign crates"
    );
    assert_eq!(
        foreign_crate_in("use std::collections::BTreeMap;"),
        None,
        "the standard library is not a foreign crate"
    );
    let forged = "[dependencies]\nserde.workspace = true\ndb = { path = \"../db\" }\n";
    assert!(
        is_path_dependency("db", forged),
        "a path dependency in the domain's table must be caught"
    );
    assert!(
        !is_path_dependency("serde", forged),
        "a workspace dependency is not a path dependency"
    );
}

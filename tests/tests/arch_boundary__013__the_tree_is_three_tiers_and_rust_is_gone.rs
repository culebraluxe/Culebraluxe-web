//! ARCH.BOUNDARY — the tree is three tiers, and `rust/` is gone (TST-ARCH-BOUNDARY-013).
//!
//! Contract: the layout is a fact the repository has already paid for. Before 2026-10-01 the whole application lived
//! inside `rust/`, one directory down, as though Rust were a subdirectory of the TypeScript application it replaced —
//! and that TypeScript application was itself gone. The move put the workspace where the workspace is: `Cargo.toml` at
//! the repository root, three tiers under it, and the entry points and the suite beside them.
//!
//! What this test is for. Every one of those facts is invisible to the compiler. A crate could move back under a
//! `rust/` directory, a crate could appear at the root outside every tier, the suite's cases could drift back to
//! living beside the crates they exercise, and `cargo check --workspace` would stay green while the tree quietly
//! returned to the arrangement the move undid. A layout is a contract like any other: it holds because something
//! fails when it stops holding.
//!
//! It is deliberately a *shape* test, not a policy test. It does not say which crate may depend on which — the other
//! `arch_boundary__*` cases do that. It says where the files are.
//!
//! Non-vacuous by construction: the walk asserts a floor on the crate roots it found, because a walker that read
//! nothing would report a beautiful tree.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__013__the_tree_is_three_tiers_and_rust_is_gone

use std::collections::BTreeSet;

use test_harness::source;

/// The tiers, and the directory inside each that proves it is a crate tier rather than a folder.
const TIER_ROOTS: [&str; 7] = [
    "web/src",
    "web/ui/src",
    "web/auth/src",
    "middle/model/src",
    "middle/services/src",
    "middle/workflow/src",
    "middle/apis/src",
];

/// The entry points: code that exists to be run, not to be depended on.
const ENTRY_POINTS: [&str; 2] = ["cli/src", "forge/src"];

/// The crates the workspace declares, exactly. Adding a crate means adding it here — which is the point: the guard is
/// where a new crate's tier gets decided on purpose rather than by wherever a `git mv` happened to put it.
const WORKSPACE_MEMBERS: [&str; 11] = [
    "web",
    "web/ui",
    "web/auth",
    "middle/model",
    "middle/services",
    "middle/workflow",
    "middle/apis",
    "db",
    "cli",
    "forge",
    "tests",
];

/// The container files: all of them under `devops/` except the one the Vercel build reads from the repository root.
const CONTAINER_FILES: [&str; 5] = [
    "Dockerfile",
    "devops/Dockerfile",
    "devops/Dockerfile.build",
    "devops/Dockerfile.runtime",
    "devops/Dockerfile.vercel",
];

/// The paths that must NOT exist. Each is where the old layout lived, and any one of them back is the move coming
/// undone quietly.
const GONE: [&str; 6] = [
    "rust",
    "deploy",
    "web/tests",
    "db/tests",
    "forge/tests",
    "data/skills",
];

/// A floor on the crate roots the walk must find: fewer than this means it read the wrong tree, which would make every
/// assertion in this file pass while checking nothing.
const CRATE_ROOT_FLOOR: usize = 7;

/// A floor on the case files in `tests/tests/`: the three crate test directories and the harness were 90 together.
const CASE_FLOOR: usize = 80;

fn in_repo(relative: &str) -> std::path::PathBuf {
    source::repo_root().join(relative)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-013); the file and the assay use it.
fn arch_boundary_013__the_tree_is_three_tiers_and_rust_is_gone() {
    // 1. THE TIERS AND THE ENTRY POINTS EXIST, and each is a crate with sources rather than an empty folder.
    let mut crate_roots = 0;
    let mut missing: Vec<&str> = Vec::new();
    for relative in TIER_ROOTS.iter().chain(ENTRY_POINTS.iter()) {
        if in_repo(relative).is_dir() {
            crate_roots += 1;
        } else {
            missing.push(relative);
        }
    }
    assert!(
        missing.is_empty(),
        "the tier layout is gone for: {missing:?}. Every Rust path in this repository is spelled from the root, and a \
         tier that is not where it is spelled is a tree that has drifted back"
    );
    assert!(
        crate_roots >= CRATE_ROOT_FLOOR,
        "the walk found {crate_roots} crate roots (floor {CRATE_ROOT_FLOOR}) — a walker reading the wrong tree would \
         report a layout that is not there"
    );

    // 2. THE WORKSPACE DECLARES EXACTLY THESE CRATES. The manifest is the one file that decides what a build sees, so
    //    a crate outside it compiles nowhere — and a member list nobody re-reads is how that happens.
    let manifest = source::read(&in_repo("Cargo.toml"));
    let members: BTreeSet<String> = {
        // The quoted names between `members = [` and its `]`, rather than the file's lines: a member list that grows
        // multi-line formatting is still the same list of crates.
        let start = manifest
            .find("members = [")
            .expect("the workspace declares its members");
        let rest = &manifest[start..];
        let end = rest.find(']').expect("the members list closes");
        rest[..end]
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    };
    let declared: BTreeSet<String> = WORKSPACE_MEMBERS
        .iter()
        .map(|one| one.to_string())
        .collect();
    assert_eq!(
        members, declared,
        "the workspace members changed. A new crate belongs in a tier (`web/`, `middle/`, `db/`), in the entry points \
         (`cli/`, `forge/`) or in the suite (`tests/`) — and then in WORKSPACE_MEMBERS in this test, so the decision \
         is made where a reader can see it"
    );
    for member in &declared {
        assert!(
            in_repo(&format!("{member}/Cargo.toml")).is_file(),
            "{member} is a workspace member with no manifest"
        );
    }

    // 3. THE SUITE IS ONE CRATE AT THE ROOT: `tests/src/` is the harness, `tests/tests/` the cases, and no crate keeps
    //    a `tests/` directory of its own — a case beside its crate is how the estate grew three sizes in the first
    //    place, and section 4 pins the three directories it grew in.
    assert!(
        in_repo("tests/Cargo.toml").is_file(),
        "the contract suite has no manifest at tests/Cargo.toml"
    );
    for relative in ["tests/src/lib.rs", "tests/tests/support"] {
        assert!(
            in_repo(relative).exists(),
            "the suite is not one crate: {relative} is part of what makes one of it"
        );
    }
    let cases = std::fs::read_dir(in_repo("tests/tests"))
        .expect("tests/tests must be readable — it is where every case lives")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".rs"))
        .count();
    assert!(
        cases >= CASE_FLOOR,
        "the suite holds {cases} case files (floor {CASE_FLOOR}): the harness and the three crate test directories \
         were 90 together, so a count this low means cases were left behind"
    );

    // 4. THE PATHS THE OLD LAYOUT LIVED AT ARE GONE. A tree that keeps `rust/` keeps a second answer to \"where does
    //    the code live\", and the second answer is the one an agent reads by accident.
    let present: Vec<&str> = GONE
        .iter()
        .copied()
        .filter(|relative| in_repo(relative).exists())
        .collect();
    assert!(
        present.is_empty(),
        "these paths are from the layout the tree left: {present:?}. The CI workflows, the scripts and the hooks all \
         spell their paths from the root now, so they read the tree this guard describes — not the one that is there"
    );

    // 5. THE CONTAINER FILES ARE UNDER `devops/`, and the nextest profile is where nextest looks for it: the workspace
    //    root. A profile left under a crate is a `--profile ci` that quietly means nothing.
    for relative in CONTAINER_FILES {
        assert!(
            in_repo(relative).is_file(),
            "the container file {relative} is not where the docs and the deploy scripts say it is"
        );
    }
    let nextest = in_repo(".config/nextest.toml");
    assert!(
        nextest.is_file(),
        "the nextest profile must sit at the workspace root (.config/nextest.toml): without it `cargo nextest run \
         --profile ci` runs with no profile and publishes no JUnit report"
    );
    assert!(
        source::read(&nextest).contains("[profile.ci]"),
        "the profile file is there but no longer defines `ci`"
    );

    // 6. BUILD OUTPUT IS OUTSIDE THE SOURCE TREE, and the wasm build defaults there. `target/` at the root is compiler
    //    output the repository ignores; a `rust/target` would be the old workspace's, kept where it no longer is.
    let ui_build = source::read(&in_repo("scripts/rust-ui-build.sh"));
    assert!(
        !ui_build.contains("rust/target"),
        "the wasm build still defaults its target directory to a path under rust/, which the tree no longer has"
    );
    assert!(
        !in_repo("rust/target").exists(),
        "a rust/target directory is a workspace that moved but kept its build output where it was"
    );

    // 7. AND THE WORKSPACE ROOT IS THE ONLY ONE. `Cargo.toml` beside `cargo`'s own directory, not nested in a crate:
    //    nested, `cargo check --workspace` from the root would silently compile a subset and report success.
    assert!(
        in_repo("Cargo.toml").is_file() && in_repo("Cargo.lock").is_file(),
        "the workspace manifest and its lock live at the repository root"
    );
    assert!(
        !in_repo("web/Cargo.lock").exists() && !in_repo("middle/Cargo.lock").exists(),
        "a second lockfile means a second workspace"
    );
}

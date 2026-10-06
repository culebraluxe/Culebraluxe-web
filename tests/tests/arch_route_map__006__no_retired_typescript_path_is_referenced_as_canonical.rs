//! ARCH.ROUTE_MAP — no retired TypeScript path is referenced as canonical (TST-ARCH-ROUTE-MAP-006).
//!
//! Contract: `scripts/rust-parity-map.json` and its generated ledger `docs/rust-parity-ledger.md` may not present a
//! retired TypeScript path as the thing that serves production. The TypeScript application was deleted in 4cf98110;
//! the map's own `note` records the consequence ("`productionPath` is 'rust' wherever a route is mounted, because the
//! Rust server is now the only implementation that exists") and `docs/agent/BROKEN-TS-INVENTORY.md` records what is
//! left of that estate: dead files marked `⚠ BROKEN ON PURPOSE`, to be translated, never revived. A dead path cannot
//! serve anything, so naming one as canonical is not a stale claim — it is a claim provably false against a tree
//! where `legacy/` does not exist.
//!
//! What "canonical" means here, structurally rather than by prose: the map's canonical fields are `productionPath`
//! (the serving claim), `rustModules` (the implementation it points at) and the `module`/`routes` entries of
//! `nativeRoutes`; the ledger's canonical fields are the `serving production` column of its capability table and the
//! source claims in its `## The live Rust surface` list. Prose is deliberately NOT scanned: the map's `note` and the
//! ledger's header *name* `scripts/rust-parity-ledger.ts` (the retired generator) in order to explain how the file
//! was produced, and a sentence naming a file is not a claim that the file serves traffic — the same shape rule
//! `arch_boundary__012__retired_ts_trees_stay_retired.rs` uses for imports. Only the fields are claims.
//!
//! Pinned by reading both files on 2026-10-05: 14 capabilities, 42 `rustModules`, **0** `tsFiles` entries in total
//! (every capability's array is empty — there is no TypeScript left to move), 12 `productionPath: "rust"` + 2
//! `"none"`, 14 ledger capability rows whose `serving production` column is `rust`/`none` and whose `TS files still
//! in play` column is `—`, 113 route bullets in the ledger's live surface, and 3 module counts of **0** under
//! "TypeScript modules under the subjects". Any non-zero of those is the retirement being walked back.
//!
//! Non-vacuous by construction: four planted violations (a `.ts` production path, the `typescript` status, a dead
//! `legacy/` file still listed as in play, and a ledger row whose serving column says `typescript`) are fed to the
//! same detectors before those detectors are trusted with the real files.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network, nothing written.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical

use std::fs;
use std::path::{Path, PathBuf};

/// Script extensions: a path carrying any of them is a JavaScript/TypeScript file, and none of those can be the
/// implementation that serves production in a tree whose application is Rust.
const SCRIPT_EXTENSIONS: [&str; 6] = ["ts", "tsx", "mts", "mjs", "js", "jsx"];

/// The retired tree. It was removed from disk on 2026-10-02 (`docs/agent/DEAD-TS-DOWNSIZE.md` §5), so any reference
/// to a path under it names a file that cannot exist — which is what makes such a reference provably dead rather
/// than merely old.
const RETIRED_TREE: &str = "legacy";

/// Floors read from `scripts/rust-parity-map.json` on 2026-10-05: 14 capabilities carrying 42 `rustModules`, and 23
/// native areas each naming a `.rs` module. A reader that found fewer is reading the wrong file, and a reader that
/// reads nothing reports a spotless map.
const CAPABILITY_FLOOR: usize = 14;
const RUST_MODULE_FLOOR: usize = 42;
const NATIVE_AREA_FLOOR: usize = 23;

/// Floors read from `docs/rust-parity-ledger.md` on the same day: 14 capability rows and 113 route bullets under
/// `## The live Rust surface`.
const LEDGER_ROW_FLOOR: usize = 14;
const LEDGER_ROUTE_FLOOR: usize = 113;

/// The repository root — resolved HERE, not through `test_harness::source::repo_root()`. The cargo target dir is
/// shared across lanes and `tests/src/*.rs` is byte-identical in all of them, so the `test_harness` rlib may be a
/// sibling lane's artifact whose `env!("CARGO_MANIFEST_DIR")` bakes ANOTHER lane's path in; that made a test measure
/// the wrong checkout on 2026-10-04. The same expression, evaluated in this file's own compilation.
fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("the repository root must resolve")
}

/// A reference that points at retired TypeScript as though it were a live implementation: the `typescript` status
/// itself, anything under `legacy/`, or a path whose extension is a script.
fn is_retired_ts_reference(value: &str) -> bool {
    let value = value.trim().trim_matches('`').trim();
    if value.is_empty() {
        return false;
    }
    if value.eq_ignore_ascii_case("typescript") {
        return true;
    }
    let path = value.replace('\\', "/");
    if path.starts_with(&format!("{RETIRED_TREE}/")) || path.contains(&format!("/{RETIRED_TREE}/"))
    {
        return true;
    }
    let last = path.rsplit('/').next().unwrap_or(&path);
    let last = last.split(['?', '#']).next().unwrap_or(last);
    match last.rfind('.') {
        Some(dot) => SCRIPT_EXTENSIONS.contains(&&last[dot + 1..]),
        None => false,
    }
}

/// One capability row of the map, reduced to the fields this contract reads.
struct Capability {
    id: String,
    production_path: String,
    rust_modules: Vec<String>,
    ts_files: Vec<String>,
}

fn strings(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| entry.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn capabilities(map: &serde_json::Value) -> Vec<Capability> {
    map["capabilities"]
        .as_array()
        .expect("capabilities must be an array")
        .iter()
        .map(|capability| Capability {
            id: capability["id"]
                .as_str()
                .expect("every capability carries an id")
                .to_string(),
            production_path: capability["productionPath"]
                .as_str()
                .expect("every capability declares what serves production")
                .to_string(),
            rust_modules: strings(&capability["rustModules"]),
            ts_files: strings(&capability["tsFiles"]),
        })
        .collect()
}

/// The serving claim, judged: `Some(reason)` when the capability says a retired TypeScript path answers for it.
fn serving_path_is_retired(capability: &Capability) -> Option<String> {
    if is_retired_ts_reference(&capability.production_path) {
        return Some(format!(
            "{} declares productionPath \"{}\" — the TypeScript application was deleted in 4cf98110 and `legacy/` \
             is not in the tree, so nothing of that name can serve anything",
            capability.id, capability.production_path
        ));
    }
    if capability.production_path.contains('/') || capability.production_path.contains('\\') {
        return Some(format!(
            "{} declares productionPath \"{}\": productionPath is the status of what serves production, not a file \
             to point at; a file path there is how a retired path becomes canonical",
            capability.id, capability.production_path
        ));
    }
    None
}

/// The `tsFiles` half: `Some(reason)` when a TypeScript file is presented as in play — it is the serving path, or it
/// names a file that does not exist (a dead path cannot be "still in play").
fn ts_files_are_in_play(capability: &Capability, root: &Path) -> Option<String> {
    for entry in &capability.ts_files {
        if entry == &capability.production_path {
            return Some(format!(
                "{} lists {entry} as both a tsFile and its productionPath: a retired TypeScript file is being \
                 presented as the serving implementation",
                capability.id
            ));
        }
        if entry.contains('/') && !root.join(entry).exists() {
            return Some(format!(
                "{} lists tsFile {entry}, which does not exist in this tree — a path under {} cannot be in play, \
                 because that tree was removed on 2026-10-02",
                capability.id, RETIRED_TREE
            ));
        }
    }
    None
}

/// One row of the ledger's capability table.
struct LedgerRow {
    id: String,
    serving: String,
    ts_files: String,
}

/// The capability table of `docs/rust-parity-ledger.md`: `| \`clients\` | built | rust | 4 | — |`.
fn ledger_rows(ledger: &str) -> Vec<LedgerRow> {
    ledger
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            assert!(
                cells.len() >= 7,
                "a capability row of the ledger no longer has five columns: {line}"
            );
            LedgerRow {
                id: cells[1].trim_matches('`').to_string(),
                serving: cells[3].to_string(),
                ts_files: cells[5].to_string(),
            }
        })
        .collect()
}

/// The route claims of the ledger's `## The live Rust surface` section — `- \`/v1/clients\` _(native: …)_` — as the
/// path inside the backticks.
fn ledger_route_paths(ledger: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in ledger.lines() {
        if line.starts_with("## ") {
            inside = line.starts_with("## The live Rust surface");
            continue;
        }
        if line.starts_with("### ") {
            inside = false;
            continue;
        }
        if !inside || !line.starts_with("- `") {
            continue;
        }
        let rest = &line["- `".len()..];
        if let Some(end) = rest.find('`') {
            out.push(rest[..end].to_string());
        }
    }
    out
}

/// The module counts under `## TypeScript modules under the subjects`, which are all `**0**` — the number of
/// TypeScript modules left that anything could reach.
fn ledger_ts_module_counts(ledger: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in ledger.lines() {
        if line.starts_with("## ") {
            inside = line.starts_with("## TypeScript modules under the subjects");
            continue;
        }
        if !inside || !line.starts_with("- ") {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("**") {
            rest = &rest[start + 2..];
            let Some(end) = rest.find("**") else { break };
            if let Ok(count) = rest[..end].trim().trim_matches('*').parse::<usize>() {
                out.push((count, line.to_string()));
            }
            rest = &rest[end + 2..];
        }
    }
    out
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ROUTE-MAP-006); the file and the assay use it.
fn arch_route_map_006__no_retired_typescript_path_is_referenced_as_canonical() {
    // PLANTED NEGATIVES. The same detectors, fed four retired references that must be refused, before either is
    // trusted with the real files — a detector that accepts these would accept anything.
    assert!(
        is_retired_ts_reference("legacy/services/clients.ts"),
        "a .ts path under the retired tree must be recognised as retired TypeScript"
    );
    assert!(
        is_retired_ts_reference("typescript"),
        "the productionPath status \"typescript\" must be recognised as a claim the deleted application serves \
         production"
    );
    assert!(
        !is_retired_ts_reference("rust") && !is_retired_ts_reference("none"),
        "the two honest statuses must not be flagged"
    );
    let root = repo_root();
    assert!(
        ts_files_are_in_play(
            &Capability {
                id: "ghost".to_string(),
                production_path: "rust".to_string(),
                rust_modules: vec!["web/src/ghost".to_string()],
                ts_files: vec!["legacy/db/ghost.ts".to_string()],
            },
            &root,
        )
        .is_some(),
        "a tsFile naming a file under the retired tree must be refused: it cannot be in play, the tree is gone"
    );
    assert!(
        serving_path_is_retired(&Capability {
            id: "ghost".to_string(),
            production_path: "legacy/services/ghost.ts".to_string(),
            rust_modules: vec![],
            ts_files: vec![],
        })
        .is_some(),
        "a productionPath that IS a retired TypeScript file must be refused"
    );
    let planted_row = "| `ghost` | built | typescript | 1 | `legacy/ghost.ts` |";
    let parsed = ledger_rows(planted_row);
    assert_eq!(parsed.len(), 1, "the planted row must parse as one row");
    assert!(
        is_retired_ts_reference(&parsed[0].serving) || parsed[0].ts_files.contains(RETIRED_TREE),
        "a ledger row whose serving column is `typescript` and whose TS column names a legacy file must be refused"
    );

    // The retired tree must be absent — that is what makes every legacy path in these files provably dead rather
    // than merely old.
    let retired = root.join(RETIRED_TREE);
    assert!(
        !retired.exists(),
        "{} is back on disk; until it is gone, a path under it is not provably dead, and this contract's whole \
         premise fails",
        retired.display()
    );

    // The map.
    let map_path = root.join("scripts/rust-parity-map.json");
    let map: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&map_path)
            .expect("the parity map must exist — it is the subject of this contract"),
    )
    .expect("the parity map must be valid JSON");

    let mut rust_modules = 0;
    let mut ts_files_total = 0;
    let mut serving_claims = 0;
    let mut retired_in_map: Vec<String> = Vec::new();
    let capabilities = capabilities(&map);
    assert!(
        capabilities.len() >= CAPABILITY_FLOOR,
        "the map declares {} capabilities (floor {CAPABILITY_FLOOR} as read on 2026-10-05)",
        capabilities.len()
    );
    for capability in &capabilities {
        if let Some(reason) = serving_path_is_retired(capability) {
            retired_in_map.push(reason);
        }
        if let Some(reason) = ts_files_are_in_play(capability, &root) {
            retired_in_map.push(reason);
        }
        for module in &capability.rust_modules {
            rust_modules += 1;
            if is_retired_ts_reference(module) {
                retired_in_map.push(format!(
                    "{} names {module} as its implementation: a retired TypeScript path presented as canonical",
                    capability.id
                ));
                continue;
            }
            if !root.join(module).exists() {
                retired_in_map.push(format!(
                    "{} names {module}, which does not exist in this tree",
                    capability.id
                ));
            }
        }
        ts_files_total += capability.ts_files.len();
        if capability.production_path == "rust" || capability.production_path == "none" {
            serving_claims += 1;
        }
    }
    assert!(
        rust_modules >= RUST_MODULE_FLOOR,
        "the map names {rust_modules} rustModules (floor {RUST_MODULE_FLOOR} as read on 2026-10-05); a reader that \
         found fewer is reading the wrong file"
    );
    assert_eq!(
        serving_claims,
        capabilities.len(),
        "every capability must declare rust/none; a status outside those two has to be re-derived by hand: \
         {retired_in_map:#?}"
    );
    assert_eq!(
        ts_files_total, 0,
        "the map's own note says there is no TypeScript left to move, so every tsFiles array is empty; {} entr(y/ies) \
         are now claimed to be in play: {retired_in_map:#?}",
        ts_files_total
    );

    let native = map["nativeRoutes"]
        .as_array()
        .expect("nativeRoutes must be an array");
    assert!(
        native.len() >= NATIVE_AREA_FLOOR,
        "the map declares {} native areas (floor {NATIVE_AREA_FLOOR} as read on 2026-10-05)",
        native.len()
    );
    for area in native {
        let area_name = area["area"].as_str().expect("an area carries a name");
        let module = area["module"].as_str().expect("an area names its module");
        if is_retired_ts_reference(module) {
            retired_in_map.push(format!(
                "native area `{area_name}` names {module} as its module: a retired TypeScript path presented as \
                 canonical"
            ));
        }
        if !root.join(module).exists() {
            retired_in_map.push(format!(
                "native area `{area_name}` names {module}, which does not exist in this tree"
            ));
        }
        for route in area["routes"].as_array().expect("an area lists routes") {
            let route = route.as_str().expect("a route is a string");
            if is_retired_ts_reference(route) {
                retired_in_map.push(format!(
                    "native area `{area_name}` claims route {route}: a script path is not an API route"
                ));
            }
        }
    }
    for route in map["knownUnmappedRoutes"]
        .as_array()
        .expect("knownUnmappedRoutes must be an array")
    {
        let route = route.as_str().expect("a route is a string");
        if is_retired_ts_reference(route) {
            retired_in_map.push(format!(
                "knownUnmappedRoutes claims {route}: a script path is not an API route"
            ));
        }
    }
    assert!(
        retired_in_map.is_empty(),
        "retired TypeScript is presented as canonical in scripts/rust-parity-map.json: {retired_in_map:#?}. The \
         TypeScript application was deleted in 4cf98110; translate behaviour into Rust, never point at a file that \
         is no longer in the tree"
    );

    // The ledger, whose capability table is the answer an operator reads in an incident.
    let ledger = fs::read_to_string(root.join("docs/rust-parity-ledger.md"))
        .expect("the ledger must exist — it is the generated view of the map this contract reads");
    let rows = ledger_rows(&ledger);
    assert!(
        rows.len() >= LEDGER_ROW_FLOOR,
        "the ledger's capability table has {} rows (floor {LEDGER_ROW_FLOOR} as read on 2026-10-05)",
        rows.len()
    );
    let mut retired_in_ledger: Vec<String> = Vec::new();
    for row in &rows {
        if is_retired_ts_reference(&row.serving) || row.serving.contains('/') {
            retired_in_ledger.push(format!(
                "row `{}` says `{}` serves production: a retired TypeScript path may not be the serving column",
                row.id, row.serving
            ));
        }
        if row.ts_files.trim() != "—" && !row.ts_files.trim().is_empty() {
            retired_in_ledger.push(format!(
                "row `{}` lists TS files still in play: `{}` — the map behind the ledger carries no TypeScript \
                 that could be in play",
                row.id, row.ts_files
            ));
        }
    }
    let routes = ledger_route_paths(&ledger);
    assert!(
        routes.len() >= LEDGER_ROUTE_FLOOR,
        "the ledger lists {} routes under `## The live Rust surface` (floor {LEDGER_ROUTE_FLOOR} as read on \
         2026-10-05)",
        routes.len()
    );
    for route in &routes {
        if is_retired_ts_reference(route) {
            retired_in_ledger.push(format!(
                "the live surface claims {route}: a script path may not be presented as a served route"
            ));
        }
    }
    for (count, line) in ledger_ts_module_counts(&ledger) {
        if count != 0 {
            retired_in_ledger.push(format!(
                "the ledger reports {count} TypeScript module(s) under `## TypeScript modules under the subjects`: \
                 {line}"
            ));
        }
    }
    assert!(
        retired_in_ledger.is_empty(),
        "retired TypeScript is presented as canonical in docs/rust-parity-ledger.md: {retired_in_ledger:#?}"
    );
}

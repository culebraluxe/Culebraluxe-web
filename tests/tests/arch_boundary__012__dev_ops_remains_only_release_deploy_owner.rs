//! ARCH.BOUNDARY — DEV_OPS remains the only release/deploy owner (TST-ARCH-BOUNDARY-012).
//!
//! CONTRACT. `DevOps` is the release lane: service `forge.devops` (the nodes `deploy`, `production_smoke`
//! and `repair_devops`) and position `dev_ops` (those three plus the engine's eight release commands —
//! publish, both migrations, the derived refresh). This test states that ownership in BOTH directions — a
//! second owner anywhere fails here, and so does the owner set moving without this file being edited:
//!
//!   1. THE LANE'S NODES, read live rather than from text. `FORGE_SDLC-v6.xml` binds every executable
//!      task-node to a service and a service is a lane, so the release nodes are whatever `forge.devops`
//!      owns: exactly the three pinned below; each resolved by the registry to exactly ONE owner (the
//!      registry refuses a second owner for a lane and a second service for a node); each refused by the
//!      other six services; and the lane module's own `RELEASE_NODES` constant naming the same three. The
//!      definition's `responsibility="dev_ops"` POSITION is larger — eleven nodes: those three task-nodes
//!      plus eight command-nodes (publish, both migrations, the derived refresh and their verifications),
//!      every one of them a release act — and it is pinned in both directions too, with the split asserted:
//!      the position's agent nodes ARE the service's three, and no release-named work node sits in another
//!      position. NO node bound to any other service carries a release segment in its name (`deploy`,
//!      `release`, `publish`, `smoke`, `production`, `vercel`, `devops`, `dev_ops`) — the foreign-owner
//!      matcher is exercised against a planted second owner below, so a release-named node added to another
//!      lane fails rather than sits unnoticed. Finally the failure route: the repair owner of a
//!      `DEPLOYMENT_FAILURE` (and of an `ENVIRONMENT_FAILURE`) is the `dev_ops` position, and no other
//!      failure class routes there.
//!   2. THE RELEASE HANDLE. Across every production `.rs` file, exactly one module EXPORTS a concrete
//!      release executor (`pub use …HostReleaseExecutor|DbForgeReleaseExecutor`) — the lane's own
//!      `forge/src/roles/dev_ops.rs` — and exactly one site BUILDS one — the composition root
//!      `forge/src/bin/forge.rs`. The six other lane modules (and the rest of `forge/src/roles/`) name none
//!      of the ten release-owner tokens. The handle then reaches the engine only as a trait object
//!      (`Arc<dyn ForgeReleaseExecutor>`) through four pinned engine files, none of them a lane.
//!   3. THE DOOR. Exactly one production file can invoke the publish path — `forge/src/engine/git_publish.rs`,
//!      the door itself, whose gate on `FORGE_ALLOW_PUBLISH` at the point of use TST-ARCH-BOUNDARY-011 pins
//!      (its fact 3, the one mutation verb in the workspace). Exactly three production files even READ that
//!      switch: the door gates on it, the composition root logs it, the worker forwards it to a child. The
//!      web tier — the TECH cockpit included — names none of the release-owner tokens: it reads the build
//!      stamp, it cannot deploy. And every one of the eleven agent profiles, the DevOps agent included, is
//!      DENIED the deploy shells (`git push*`, `pnpm deploy*`, `vercel*`, `kubectl*`), because publication
//!      and deployment are Forge-owned code paths, never a model's shell: `scripts/deploy-prod.sh` is a
//!      Captain's production action.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * **The `dev_ops` POSITION is eleven nodes; the `forge.devops` SERVICE is three.** Measured on
//!     2026-10-04, not assumed: the position carries `fast_publish`, `publish_candidate`, `migrate_dev`,
//!     `verify_dev_migration`, `migrate_prod`, `verify_prod_migration`, `refresh_derived_models` and
//!     `verify_derived_models` beside the three deploy nodes — the eight are COMMAND-nodes (they run
//!     through the engine's command port, `forge.publish_candidate` and friends), not agent turns, so they
//!     belong to the position rather than to a service binding. Two of them, `fast_publish` and
//!     `publish_candidate`, are literally the publish act; all eight are release work. The first draft of
//!     this test expected the two sets to be equal and failed — the divergence is recorded here and pinned
//!     in both halves rather than resolved by narrowing either pin to match the other.
//!   * **Decisions carry no responsibility.** `deploy_required`, `migration_required`,
//!     `derived_refresh_required` and `devops_resume_router` are routing questions the engine asks, not
//!     work anyone owns, so they appear in neither set and the work-node check reads task- and
//!     command-nodes only.
//!   * **`forge/src/engine/deploy.rs` says "deploy" and means something else.** It upserts a *workflow
//!     definition* into `process_definitions` (`deploy_xml`) — definition deployment, not production
//!     deployment. It holds no release token and is not a second owner; the name collision is recorded
//!     here rather than resolved by a rename.
//!   * **`tests/` is not in the production sweep.** The harness's own suite deliberately builds
//!     `DbForgeReleaseExecutor` and calls the door (`forge_publish__001`, `forge_runtime`,
//!     `forge_arch_seam__007`): those are tests OF the door, not a second owner of it. Everything else in
//!     the tree is swept, `experiments/` included, against a floor that fails if the walker finds nothing.
//!   * **Three engine files NAME the `"deploy"` node without owning it** — `facts.rs` (evidence),
//!     `role_slice.rs` (release evidence) and `failure.rs` (the failure class) read it, `dev_ops.rs` owns
//!     it. The census of that literal is pinned: a fourth production file that starts to talk about
//!     `"deploy"` is a deliberate edit here. The pin says who names it, not that nobody may.
//!   * **The trait port is plumbing, not ownership.** `ForgeReleaseExecutor` is defined in
//!     `engine/writer.rs` and re-exported by `engine/mod.rs`; `dispatch.rs`, `runtime.rs`, `port.rs` and
//!     `bin/forge.rs` carry `Arc<dyn …>`. Only the composition root builds a concrete one, and a lane
//!     naming the port at all trips the lane-module token scan below.
//!   * **`repair_devops` is release-named by the `devops` segment** and is DevOps's by binding — the
//!     foreign-owner matcher compares the OWNER, so it stays exactly where it belongs.
//!   * **`worker.rs` forwards `FORGE_ALLOW_PUBLISH` into the environment of a child it spawns.** It reads
//!     the switch; it does not publish. Publishing happens in `git_publish.rs`, gated at the point of use.
//!   * **Two files share index 012.**
//!     `arch_boundary__012__retired_ts_trees_stay_retired.rs` is a sibling from an earlier numbering; this
//!     file is the packet's canonical `…012__dev_ops_remains_only_release_deploy_owner.rs`, and neither
//!     touches the other.
//!   * **Where this deliberately overlaps another test:** `arch_boundary__011` pins the whole node → lane
//!     map and the workspace's single mutation verb; `roles/service.rs::devops_alone_owns_the_publish_nodes`
//!     proves the router refuses the publish nodes without DevOps registered; `opencode_v2_agents.rs`
//!     proves the shell denies in the RENDERED config. This test repeats none of those: it pins the OWNER
//!     SET — nodes, module, construction site, door, shells — from the live binding and a sweep, both ways.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it reads sources and the
//! in-memory definition, registry and permission renderer. It does not run the engine, does not execute
//! `scripts/deploy-prod.sh` (a shell file the `.rs` sweep does not read), does not prove the runtime value
//! of `FORGE_ALLOW_PUBLISH`, and touches no database and no network.
//!
//! Level: L0 Pure — filesystem reads, the in-memory definition/registry, and the permission renderer. No
//! database, no network, no process spawned, no file written.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__012__dev_ops_remains_only_release_deploy_owner

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::failure::{route_failure, ForgeFailureClass, ForgeFailureRouting};
use forge::engine::opencode_agents as agents;
use forge::engine::role_mapping::LaneId;
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::service_binding::{
    forge_service_bindings, lane_for_node, nodes_for_service, service_for_node,
};
use forge::engine::xml::{definition_from_xml, FORGE_SDLC_V6_XML};
use forge::roles::dev_ops::{DEVOPS_SERVICE_ID, RELEASE_NODES};
use forge::roles::service::ForgeLaneServices;
use serde_json::{json, Value};
use test_harness::source;

/// The repository root THIS compilation unit was built from: `tests/`, one level below the root.
///
/// Evaluated here rather than via `test_harness::source::repo_root()` on purpose. The cargo target dir
/// (`build/rust`) is shared by every lane of this repo and `tests/src/*.rs` is byte-identical in all of
/// them, so the `test_harness` rlib may be a sibling lane's artifact — and a sibling's
/// `env!("CARGO_MANIFEST_DIR")` bakes THAT lane's path in. Reading the tree through it measures another
/// checkout (observed 2026-10-04: a run linked a `lane-nemotron` rlib while building in `lane-mimo`, and
/// this test then failed on a file that differs between the two). The helpers that take an explicit path
/// (`source::sources_under`, `source::read`) are used as-is; only the root is resolved here.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the suite lives in tests/, one level below the repository root")
        .to_path_buf()
}

/// `path` as this tree writes it — relative to the root, for failure messages a reader can act on.
/// `source::relative` does the same against `source::repo_root()`'s (possibly foreign) root.
fn relative_to_root(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The DevOps lane's nodes, as `FORGE_SDLC-v6.xml` binds them. Pinned in both directions: a fourth release
/// node, a renamed one, or one of these landing in another service is a deliberate edit of this file.
const DEVOPS_NODES: [&str; 3] = ["deploy", "production_smoke", "repair_devops"];

/// The whole `dev_ops` POSITION in the definition — eleven nodes: the three agent task-nodes bound to
/// `forge.devops`, plus eight engine COMMAND nodes (publish, DEV/PROD migration, derived refresh) that run
/// through the engine's command port rather than a lane turn. Measured 2026-10-04 from
/// `FORGE_SDLC-v6.xml`. Pinned both ways: a release act added to another position, or one of these leaving
/// `dev_ops`, is a deliberate edit of this file.
const DEV_OPS_POSITION_NODES: [&str; 11] = [
    "deploy",
    "fast_publish",
    "migrate_dev",
    "migrate_prod",
    "production_smoke",
    "publish_candidate",
    "refresh_derived_models",
    "repair_devops",
    "verify_derived_models",
    "verify_dev_migration",
    "verify_prod_migration",
];

/// The command half of that position: the release acts the ENGINE performs itself, owned by `dev_ops` and
/// by no other responsibility in the definition. A task-node of the same name would be an agent turn; these
/// are command-nodes, so they belong to the position rather than to a lane's service binding.
const DEV_OPS_COMMAND_NODES: [&str; 8] = [
    "fast_publish",
    "migrate_dev",
    "migrate_prod",
    "publish_candidate",
    "refresh_derived_models",
    "verify_derived_models",
    "verify_dev_migration",
    "verify_prod_migration",
];

/// The seven lane modules — one per `LaneId`. A lane module renamed or added must be named here before the
/// scan below will read it; the seven are the whole lane surface.
const LANE_MODULES: [&str; 7] = [
    "forge/src/roles/architect.rs",
    "forge/src/roles/dev_ops.rs",
    "forge/src/roles/inspector.rs",
    "forge/src/roles/lead.rs",
    "forge/src/roles/qa.rs",
    "forge/src/roles/scout.rs",
    "forge/src/roles/smith.rs",
];

/// The tokens that name the release/deploy capability: the two concrete executors, their git ops, the trait
/// port, the door and its outcomes, the lane's node list, and the publish switch. A lane module (or the web
/// tier) naming ANY of them is a second claim on release.
const RELEASE_OWNER_TOKENS: [&str; 10] = [
    "HostReleaseExecutor",
    "DbForgeReleaseExecutor",
    "GitReleaseOps",
    "ForgeReleaseOperations",
    "ForgeReleaseExecutor",
    "preview_publish",
    "publish_candidate",
    "PublishOutcome",
    "RELEASE_NODES",
    "FORGE_ALLOW_PUBLISH",
];

/// The concrete executors a module may EXPORT. The trait port is engine plumbing (it is scanned separately,
/// as `dyn ForgeReleaseExecutor`, and pinned to four engine files); handing over a concrete executor is
/// handing over the handle itself.
const HANDLE_TYPES: [&str; 2] = ["HostReleaseExecutor", "DbForgeReleaseExecutor"];

/// The name segments that make a node a release node BY NAME. `devops` is included so a release node
/// planted on another lane under a DevOps-flavoured name (`smith_devops`) is caught too.
const RELEASE_NODE_SEGMENTS: [&str; 8] = [
    "deploy",
    "release",
    "publish",
    "smoke",
    "production",
    "vercel",
    "devops",
    "dev_ops",
];

/// THE PINNED OWNER SET. Derived by scanning the tree, not guessed: each constant below is compared with
/// what the sweep actually returns, so the set can only move by editing this file.
const RELEASE_HANDLE_EXPORTER: &str = "forge/src/roles/dev_ops.rs";
const RELEASE_HANDLE_WIRED: &str = "forge/src/bin/forge.rs";
const THE_ONE_DOOR: &str = "forge/src/engine/git_publish.rs";
/// Production files that READ the `FORGE_ALLOW_PUBLISH` switch itself. The door gates on it and the
/// composition root reads it; a new reader is a new actor in the release path.
///
/// `config.rs` replaced `worker.rs` on 2026-10-04, named deliberately rather than absorbed: `9bcc13a2`
/// ("config centralization") moved the read into `config.rs`, and `worker.rs` now forwards the already
/// resolved `child_cfg.allow_publish` to a child instead of reading the switch (`worker.rs:419-423`).
/// `release.rs` names the switch only in a doc comment, which the strip removes — a comment is not a
/// reader. A fourth reader still fails this pin.
const SWITCH_READERS: [&str; 3] = [
    "forge/src/bin/forge.rs",
    "forge/src/engine/config.rs",
    "forge/src/engine/git_publish.rs",
];
/// Files that hold `Arc<dyn ForgeReleaseExecutor>` and pass it into `ForgeApplicationPort` — a holder is
/// a route to a publish command, so a fifth is reported rather than absorbed.
///
/// `re_runtime.rs` joined on 2026-10-04, named deliberately rather than lost: commit `9bcc13a2` gave
/// `shared_forge_runtime(...)` the `release: Option<Arc<dyn ForgeReleaseExecutor>>` parameter and hands it
/// to `ForgeApplicationPort::new`, making the RE_supermodel host a second wiring point beside the
/// composition root. It carries the trait object; it does not own release or deploy — that stays with
/// `dev_ops`, asserted separately above. A sixth holder still fails this pin.
const RELEASE_PORT_HOLDERS: [&str; 5] = [
    "forge/src/bin/forge.rs",
    "forge/src/engine/dispatch.rs",
    "forge/src/engine/port.rs",
    "forge/src/engine/re_runtime.rs",
    "forge/src/engine/runtime.rs",
];
const DEPLOY_LITERAL_READERS: [&str; 4] = [
    "forge/src/engine/facts.rs",
    "forge/src/engine/failure.rs",
    "forge/src/engine/role_slice.rs",
    "forge/src/roles/dev_ops.rs",
];

/// Every `ForgeFailureClass`, as the enum declares them — ten, listed here because the enum exports no
/// `ALL`. [`failure_class_is_named_here`] carries the matching exhaustiveness check.
const ALL_FAILURE_CLASSES: [ForgeFailureClass; 10] = [
    ForgeFailureClass::MissingContext,
    ForgeFailureClass::BadImplementation,
    ForgeFailureClass::BadArchitecture,
    ForgeFailureClass::BadToolContract,
    ForgeFailureClass::EnvironmentFailure,
    ForgeFailureClass::MissingGuardrail,
    ForgeFailureClass::WeakTest,
    ForgeFailureClass::DependencyFailure,
    ForgeFailureClass::DeploymentFailure,
    ForgeFailureClass::Unknown,
];

/// The classes whose repair owner is the `dev_ops` position: the machine's environment and its deploys.
const DEV_OPS_FAILURE_CLASSES: [ForgeFailureClass; 2] = [
    ForgeFailureClass::EnvironmentFailure,
    ForgeFailureClass::DeploymentFailure,
];

/// The shell patterns that would let an agent fire a deploy or a publish, as `shell_rules` renders them
/// (`PUBLICATION_DENY` prefixes, each with the vendor's `*` appended).
const DEPLOY_SHELL_DENIES: [&str; 4] = ["git push*", "pnpm deploy*", "vercel*", "kubectl*"];

/// Floors. A walker that found nothing would report a clean tree and pass; these make that a failure.
/// Measured 2026-10-04: 615 production `.rs` files (259 `web/`, 112 `forge/`, 110 `middle/`, 90 `db/`,
/// 43 `cli/`, 1 `experiments/`), of which 259 are under `web/` and 12 under `forge/src/roles/`.
const PRODUCTION_SOURCE_FLOOR: usize = 500;
const WEB_SOURCE_FLOOR: usize = 200;
const ROLES_SOURCE_FLOOR: usize = 10;
const ENGINE_NODE_FLOOR: usize = 20;

/// The file doing the scanning never appears in its own sweep: `tests/` is excluded as the suite that
/// exercises the door on purpose (see the header). Stated as a constant so the exclusion is a fact, not a
/// line of code a reader has to notice.
const SWEEP_EXCLUSIONS: [&str; 1] = ["tests/"];

/// The CODE in `source_text`: `//` and `/* … */` comments removed, string and char literals copied whole.
///
/// A copy of TST-ARCH-BOUNDARY-011's stripper, because the shared seam is not mine to widen in this slice:
/// `test_harness::source::code_of` splits a line at the first `//`, which reads a `//` inside a string
/// literal as a comment and swallows the rest of the line. A scan whose subject includes
/// `env::var("FORGE_ALLOW_PUBLISH")` and door calls cannot afford either reading, so the stripper must know
/// where a literal begins. Planted samples check every behaviour below. If a third contract test needs it,
/// hoist it into `tests/src/source.rs` in its own story rather than copying a fourth time.
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

/// Copy a `'…'` char literal whole. A lifetime (`'static`) is left alone: it closes nothing, and treating
/// its apostrophe as an opening quote would eat the file.
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

/// Every production `.rs` file under the workspace root, as (path, code): comments removed, literals kept,
/// the `#[cfg(test)]` tail dropped — a file's own tests are not production code any more than `tests/` is.
///
/// The cut is by LINE (`#[cfg(test)]` as the start of a line), not by substring: `cli/src/forge/test_section.rs`
/// names the attribute inside a string of its own production code, and cutting there would drop the half of
/// that file this sweep is supposed to read. The paths are relative and sorted, so a failure lists the same
/// findings on every machine.
fn production_code() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in source::sources_under(&repo_root()) {
        let relative = relative_to_root(&path);
        if SWEEP_EXCLUSIONS
            .iter()
            .any(|excluded| relative.starts_with(*excluded))
        {
            continue;
        }
        let code = code_of_file(&source::read(&path));
        let production: String = code
            .lines()
            .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
            .map(|line| format!("{line}\n"))
            .collect();
        out.push((relative, production));
    }
    out
}

/// A `pub use …;` line that re-exports one of the concrete release executors: a module handing the release
/// handle to whoever imports it. Anchored on `pub use`, so a plain `use`, the type's own declaration and a
/// comment about exporting are none of them.
fn exports_release_handle(line: &str) -> bool {
    let line = line.trim();
    match line.strip_prefix("pub use ") {
        Some(rest) => HANDLE_TYPES.iter().any(|handle| rest.contains(handle)),
        None => false,
    }
}

/// A line that BUILDS the concrete handle — as opposed to declaring its type (`struct`) or writing its
/// methods (`impl`), which are the handle's own definition and not a second door to release.
fn builds_release_handle(line: &str) -> bool {
    line.contains("HostReleaseExecutor {") && !line.contains("struct ") && !line.contains("impl ")
}

/// A line that CALLS the publish door. The `fn` arms are excluded so the door's own definitions
/// (`pub fn publish_candidate …`, `pub fn preview_publish …`) are not read as callers, and the bare
/// identifier inside a command name (`"forge.publish_candidate"`) carries no call paren.
fn calls_publish_door(line: &str) -> bool {
    let called = line.contains("publish_candidate(") || line.contains("preview_publish(");
    called && !line.contains("fn publish_candidate") && !line.contains("fn preview_publish")
}

/// A node whose NAME says release or deploy, by segment: `qa_release` and `smith_deploy` are release nodes
/// wherever they are bound, while `qa_verify` is not one.
fn is_release_named_node(node: &str) -> bool {
    RELEASE_NODE_SEGMENTS
        .iter()
        .any(|segment| source::contains_segment(node, segment))
}

/// The nodes of a binding whose NAME says release but whose OWNER is not DEV_OPS — a second deploy owner,
/// stated as `node -> service` so the failure message names both halves of the violation.
fn foreign_release_owners(binding: &[(String, String)]) -> Vec<String> {
    binding
        .iter()
        .filter(|(node, service)| {
            is_release_named_node(node) && service.as_str() != DEVOPS_SERVICE_ID
        })
        .map(|(node, service)| format!("{node} -> {service}"))
        .collect()
}

/// The files of a sweep that export a concrete release handle.
fn handle_exporters(files: &[(String, String)]) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| code.lines().any(exports_release_handle))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The files of a sweep that BUILD the concrete handle.
fn handle_constructors(files: &[(String, String)]) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| code.lines().any(builds_release_handle))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The files of a sweep that CALL the publish door.
fn deploy_path_callers(files: &[(String, String)]) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| code.lines().any(calls_publish_door))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The files of a sweep whose code names ANY of `tokens`.
fn token_holders(files: &[(String, String)], tokens: &[&str]) -> BTreeSet<String> {
    files
        .iter()
        .filter(|(_, code)| tokens.iter().any(|token| code.contains(token)))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The files of a sweep whose code contains `literal` — the census, for one literal.
fn files_containing(files: &[(String, String)], literal: &str) -> BTreeSet<String> {
    token_holders(files, &[literal])
}

/// Whether a rendered permission block DENIES `pattern` on `bash`. Only an explicit `deny` is security: a
/// missing rule and a rule that says `allow` are the same answer to this function — `false`.
fn shell_denies(permissions: &Value, pattern: &str) -> bool {
    permissions
        .get("bash")
        .and_then(|rules| rules.get(pattern))
        .and_then(Value::as_str)
        .map_or(false, |verb| verb == "deny")
}

/// Exhaustiveness, by the compiler: no `_` arm, so a class added to `ForgeFailureClass` fails to COMPILE
/// this file until it is listed here — at which point the pin above forces its repair owner to be decided
/// rather than silently defaulted.
fn failure_class_is_named_here(class: ForgeFailureClass) {
    use ForgeFailureClass as Class;
    match class {
        Class::MissingContext
        | Class::BadImplementation
        | Class::BadArchitecture
        | Class::BadToolContract
        | Class::EnvironmentFailure
        | Class::MissingGuardrail
        | Class::WeakTest
        | Class::DependencyFailure
        | Class::DeploymentFailure
        | Class::Unknown => {}
    }
}

/// A runner the registry can be built over. It is never asked to run: this test reads structure only.
struct NoTurns;
impl ForgeRoleRunner for NoTurns {
    fn run(&self, node: &str, _: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        panic!("arch_boundary__012 reads structure and must not run {node}")
    }
}

/// The sorted names of a set of failure classes, for a comparison that does not care which order the
/// const array declared them in.
fn class_names(classes: &[ForgeFailureClass]) -> BTreeSet<&'static str> {
    classes.iter().map(|class| class.as_str()).collect()
}

#[test]
#[allow(non_snake_case)]
fn arch_boundary_012__dev_ops_remains_only_release_deploy_owner() {
    // ── 1. THE DETECTORS, against planted samples. A scan that cannot fail is not a check. ────────────────

    // The stripper, against the traps it exists to avoid: a comment that names the door is not a call, and
    // a `//` inside a string literal is not a comment.
    assert!(
        !code_of_file("/// the lane calls publish_candidate(&repo, sha) after preview_publish(&repo)\nfn f() {}\n")
            .contains("publish_candidate("),
        "a doc comment was read as code"
    );
    assert!(
        !code_of_file("/* publish_candidate(&repo, sha) */ let quorum = 0;\n")
            .contains("publish_candidate(")
            && code_of_file("/* publish_candidate(&repo, sha) */ let quorum = 0;\n")
                .contains("quorum"),
        "a block comment was read as code, or ate the statement after it"
    );
    let after_url =
        code_of_file("let endpoint = \"https://example.test///\"; let kept = publish_candidate;\n");
    assert!(
        after_url.contains("kept") && after_url.contains("publish_candidate"),
        "a `//` inside a string ate the rest of the line: {after_url:?}"
    );
    let raw = code_of_file("let sample = r#\"publish_candidate(x)\"#;\nlet quorum = 1;\n");
    assert!(
        raw.contains("quorum") && raw.contains("publish_candidate"),
        "a raw string swallowed the rest of the file: {raw:?}"
    );
    let quoted = code_of_file("let quote = '\"'; let quorum = 2;\n");
    assert!(
        quoted.contains("quorum"),
        "a quote inside a char literal swallowed the file: {quoted:?}"
    );
    let lifetime = code_of_file("fn borrow<'a>(x: &'a str) -> &'a str { x }\nlet quorum = 3;\n");
    assert!(
        lifetime.contains("quorum"),
        "a lifetime was read as a char literal: {lifetime:?}"
    );

    // The export matcher, in both directions.
    assert!(exports_release_handle(
        "pub use crate::engine::git_publish::HostReleaseExecutor;",
    ));
    assert!(exports_release_handle(
        "  pub use crate::engine::release::DbForgeReleaseExecutor;",
    ));
    assert!(!exports_release_handle(
        "use crate::engine::git_publish::HostReleaseExecutor;"
    ));
    assert!(!exports_release_handle("pub struct HostReleaseExecutor {"));
    assert!(
        !exports_release_handle("pub use crate::engine::writer::ForgeReleaseExecutor;"),
        "the trait port is not the handle: the engine carries it, and it is pinned separately below"
    );
    assert!(!exports_release_handle(
        "//! re-exports HostReleaseExecutor for the lane"
    ));

    // The construction matcher: a construction, not the type's own declaration.
    assert!(builds_release_handle(
        "    let release = Arc::new(HostReleaseExecutor {"
    ));
    assert!(!builds_release_handle("pub struct HostReleaseExecutor {"));
    assert!(!builds_release_handle("impl HostReleaseExecutor {"));

    // The door-call matcher: a call site, not the door's own `fn`, and not a command id that happens to
    // carry the same name.
    assert!(calls_publish_door(
        "        publish_candidate(&self.repo_root, sha, &proofs)"
    ));
    assert!(calls_publish_door(
        "    match preview_publish(&work, &candidate) {"
    ));
    assert!(!calls_publish_door(
        "pub fn publish_candidate(repo: &Path, candidate: &str) -> PublishOutcome {"
    ));
    assert!(!calls_publish_door(
        "pub fn preview_publish(repo: &Path, candidate: &str) -> PublishOutcome {"
    ));
    assert!(!calls_publish_door(
        "            \"forge.publish_candidate\" => self.publish(&envelope, &evidence),"
    ));

    // The node-name matcher, in both directions.
    assert!(is_release_named_node("deploy"));
    assert!(is_release_named_node("production_smoke"));
    assert!(
        is_release_named_node("repair_devops"),
        "the `devops` segment is a release segment — this node is DevOps's own, and the OWNER decides"
    );
    assert!(is_release_named_node("qa_release"));
    assert!(is_release_named_node("smith_deploy"));
    assert!(!is_release_named_node("qa_verify"));
    assert!(!is_release_named_node("lead_pre"));

    // THE NEGATIVE CASE. Planted second owners, through the very matchers the real sweep runs below: a lane
    // module that exports the handle, a production file that calls the door, and a release-named node bound
    // to another lane. If any of these three were NOT caught, the owner-set assertions further down would
    // prove nothing.
    let planted: Vec<(String, String)> = vec![
        (
            "forge/src/roles/inspector.rs".to_string(),
            code_of_file("pub use crate::engine::git_publish::HostReleaseExecutor;\n"),
        ),
        (
            "forge/src/engine/rogue.rs".to_string(),
            code_of_file("fn ship() {\n    publish_candidate(&repo, sha, &proofs);\n}\n"),
        ),
        (
            "forge/src/roles/scout.rs".to_string(),
            code_of_file("const PLAN: [&str; 1] = [\"research_release\"];\n"),
        ),
        (
            "forge/src/engine/door_definition.rs".to_string(),
            code_of_file("pub fn publish_candidate(repo: &Path, candidate: &str) -> PublishOutcome {\n    todo!()\n}\n"),
        ),
    ];
    assert_eq!(
        handle_exporters(&planted),
        BTreeSet::from(["forge/src/roles/inspector.rs".to_string()]),
        "a planted module exporting the release handle was not caught"
    );
    assert_eq!(
        deploy_path_callers(&planted),
        BTreeSet::from(["forge/src/engine/rogue.rs".to_string()]),
        "a planted second caller of the publish door was not caught (the door's own `fn` must not count)"
    );
    assert_eq!(
        handle_constructors(&planted),
        BTreeSet::new(),
        "a planted file that only DEFINES the handle must not read as a construction"
    );
    assert_eq!(
        foreign_release_owners(&[
            ("deploy".to_string(), DEVOPS_SERVICE_ID.to_string()),
            ("qa_release".to_string(), "forge.assay".to_string()),
            ("smith_deploy".to_string(), "forge.smith".to_string()),
            ("qa_verify".to_string(), "forge.assay".to_string()),
        ]),
        vec![
            "qa_release -> forge.assay".to_string(),
            "smith_deploy -> forge.smith".to_string(),
        ],
        "a release-named node on another lane was not caught"
    );
    assert_eq!(
        foreign_release_owners(&[("deploy".to_string(), "forge.smith".to_string())]),
        vec!["deploy -> forge.smith".to_string()],
        "a release node MOVED to another lane was not caught by name"
    );
    assert!(
        foreign_release_owners(&[
            ("deploy".to_string(), DEVOPS_SERVICE_ID.to_string()),
            ("repair_devops".to_string(), DEVOPS_SERVICE_ID.to_string()),
            (
                "production_smoke".to_string(),
                DEVOPS_SERVICE_ID.to_string()
            ),
        ])
        .is_empty(),
        "the release lane's own nodes must not read as foreign owners"
    );

    // The permission matcher, in both directions: only an explicit `deny` is security.
    assert!(shell_denies(
        &json!({"bash": {"vercel*": "deny", "*": "deny"}}),
        "vercel*"
    ));
    assert!(
        !shell_denies(
            &json!({"bash": {"vercel*": "allow", "*": "allow"}}),
            "vercel*"
        ),
        "a planted allow must not read as a deny"
    );
    assert!(
        !shell_denies(&json!({"bash": {"*": "deny"}}), "vercel*"),
        "a pattern no rule mentions is not a deny either"
    );
    assert!(!shell_denies(&json!({"edit": "deny"}), "vercel*"));

    // ── 2. THE SWEEP, with floors: a walker that finds nothing reports a clean tree and passes. ──────────

    let production = production_code();
    assert!(
        production.len() >= PRODUCTION_SOURCE_FLOOR,
        "the sweep read {} production `.rs` files (floor {PRODUCTION_SOURCE_FLOOR})",
        production.len()
    );
    for excluded in SWEEP_EXCLUSIONS {
        assert!(
            !production.iter().any(|(path, _)| path.starts_with(excluded)),
            "`{excluded}` is the harness's own suite — it exercises the door on purpose and must stay out \
             of the production sweep"
        );
    }

    // ── 3. THE LANE: the release nodes belong to DEV_OPS and to nothing else. ────────────────────────────

    let runner = NoTurns;
    let services = ForgeLaneServices::new(&runner);
    let registry = services
        .registry()
        .expect("the seven lane services register, one owner per key and per lane");
    assert_eq!(
        LaneId::for_service_key(DEVOPS_SERVICE_ID),
        Some(LaneId::DevOps),
        "the release service key must resolve to the DevOps lane"
    );

    // (a) The lane's node set, read from the live binding — both directions against the pin.
    let devops_nodes: BTreeSet<&str> = nodes_for_service(DEVOPS_SERVICE_ID);
    assert_eq!(
        devops_nodes,
        BTreeSet::from(DEVOPS_NODES),
        "the nodes bound to {DEVOPS_SERVICE_ID} drifted from the pin. A NEW RELEASE NODE IS SCANNED, NOT \
         ALLOWED: add it to DEVOPS_NODES here, which is the deliberate act of saying the release lane grew"
    );
    assert_eq!(
        &RELEASE_NODES[..],
        &DEVOPS_NODES[..],
        "the lane module's own RELEASE_NODES no longer names exactly the nodes the definition binds to it"
    );

    // (b) The definition's `dev_ops` POSITION names eleven nodes in both directions — the three agent
    // task-nodes above and eight engine command-nodes — and no release-named WORK node sits in another
    // position. (Decisions carry no responsibility at all: `deploy_required` and `devops_resume_router` are
    // engine routing, not owned work, so the work-node check below is over task- and command-nodes.)
    let graph = definition_from_xml(FORGE_SDLC_V6_XML)
        .expect("the definition parses")
        .definition;
    let dev_ops_position: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|(_, def)| def.responsibility.as_deref() == Some("dev_ops"))
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        dev_ops_position,
        BTreeSet::from(DEV_OPS_POSITION_NODES),
        "the nodes carrying responsibility=\"dev_ops\" and the pin have drifted. The release position owns \
         publish, both migrations and the derived refresh beside the three deploy nodes; a release act \
         moving to another position — or a new one appearing anywhere — is a deliberate edit here"
    );
    let position_tasks: BTreeSet<&str> = dev_ops_position
        .iter()
        .copied()
        .filter(|id| graph.nodes[*id].node_type == "task")
        .collect();
    assert_eq!(
        position_tasks,
        BTreeSet::from(DEVOPS_NODES),
        "the position's AGENT nodes must be exactly the nodes the release service binds"
    );
    let position_commands: BTreeSet<&str> = dev_ops_position
        .iter()
        .copied()
        .filter(|id| graph.nodes[*id].node_type == "command")
        .collect();
    assert_eq!(
        position_commands,
        BTreeSet::from(DEV_OPS_COMMAND_NODES),
        "the position's ENGINE commands (publish, migrations, derived refresh) drifted from the pin"
    );
    for (id, def) in &graph.nodes {
        if def.node_type != "task" && def.node_type != "command" {
            continue;
        }
        if !is_release_named_node(id) {
            continue;
        }
        assert_eq!(
            def.responsibility.as_deref(),
            Some("dev_ops"),
            "`{id}` is release-named work ({}) but belongs to another position ({:?}) — DEV_OPS must remain \
             the only owner of release and deploy",
            def.node_type,
            def.responsibility
        );
    }

    // (c) Each release node has exactly ONE owner — the registry refuses a second — and the other six
    // services refuse it. `resolve_node` fails on multiple owners, so a service added beside DevOps for one
    // of these nodes is an error here rather than a silent second door.
    let other_lanes: Vec<&'static str> = LaneId::ALL
        .iter()
        .filter(|lane| **lane != LaneId::DevOps)
        .map(|lane| lane.service_key())
        .collect();
    assert_eq!(other_lanes.len(), 6, "six lanes are not the release lane");
    for node in DEVOPS_NODES {
        assert_eq!(
            service_for_node(node),
            Ok(Some(DEVOPS_SERVICE_ID)),
            "{node} must bind to {DEVOPS_SERVICE_ID}"
        );
        assert_eq!(
            lane_for_node(node).ok(),
            Some(LaneId::DevOps),
            "{node} must land in the DevOps lane"
        );
        let owner = registry
            .resolve_node(node)
            .unwrap_or_else(|e| panic!("{node} has no single registered owner: {e}"));
        assert_eq!(owner.descriptor().service_id, DEVOPS_SERVICE_ID, "{node}");
        assert_eq!(owner.descriptor().lane, LaneId::DevOps, "{node}");
        assert!(
            owner.supports_node(node),
            "{node}: its owner must accept it"
        );
        for key in &other_lanes {
            let service = registry
                .resolve(key)
                .unwrap_or_else(|e| panic!("{key} registers: {e}"));
            assert!(
                !service.supports_node(node),
                "{key} also claims {node} — DEV_OPS must remain the only owner of a release node"
            );
        }
    }

    // (d) No node bound to ANY other service carries a release name in its own right.
    let binding: Vec<(String, String)> = forge_service_bindings()
        .unwrap_or_else(|error| panic!("the definition binds node -> service: {error}"))
        .iter()
        .map(|(node, service)| (node.clone(), service.clone()))
        .collect();
    assert!(
        binding.len() >= ENGINE_NODE_FLOOR,
        "the definition binds only {} nodes (floor {ENGINE_NODE_FLOOR})",
        binding.len()
    );
    let foreign = foreign_release_owners(&binding);
    assert!(
        foreign.is_empty(),
        "a release-named node is bound to a service other than {DEVOPS_SERVICE_ID}: {foreign:?}"
    );

    // (e) The failure route: a deploy that breaks is repaired by the dev_ops position, and no other class
    // routes there. Enumerated over every class, each one first checked against the compile-time exhaust.
    for class in ALL_FAILURE_CLASSES {
        failure_class_is_named_here(class);
    }
    let dev_ops_owned: BTreeSet<&'static str> = ALL_FAILURE_CLASSES
        .iter()
        .copied()
        .filter(|class| {
            matches!(
                route_failure(*class, 0, 3),
                ForgeFailureRouting::Repair { owner, .. } if owner == "dev_ops"
            )
        })
        .map(|class| class.as_str())
        .collect();
    assert_eq!(
        dev_ops_owned,
        class_names(&DEV_OPS_FAILURE_CLASSES),
        "the classes whose repair owner is the `dev_ops` position changed — release/deploy failures are \
         DEV_OPS's to repair, and no other class may be routed there"
    );
    assert!(
        matches!(
            route_failure(ForgeFailureClass::DeploymentFailure, 0, 3),
            ForgeFailureRouting::Repair { owner, .. } if owner == "dev_ops"
        ),
        "a DEPLOYMENT_FAILURE must be repaired by the dev_ops position"
    );

    // ── 4. THE HANDLE: one exporter, one construction site, four port files, no lane module in between. ──

    let exporters = handle_exporters(&production);
    assert_eq!(
        exporters,
        BTreeSet::from([RELEASE_HANDLE_EXPORTER.to_string()]),
        "the concrete release handle is exported by {exporters:?} instead of solely by \
         {RELEASE_HANDLE_EXPORTER}. A NEW EXPORTER IS SCANNED, NOT ALLOWED: a module exporting a concrete \
         executor hands the release handle to whoever imports it, which is DEV_OPS's alone"
    );

    let constructors = handle_constructors(&production);
    assert_eq!(
        constructors,
        BTreeSet::from([RELEASE_HANDLE_WIRED.to_string()]),
        "the release handle is built by {constructors:?} instead of solely by the composition root \
         {RELEASE_HANDLE_WIRED} — a lane or a route that builds its own handle is a second door to release"
    );

    for module in LANE_MODULES {
        assert!(
            production.iter().any(|(path, _)| path == module),
            "the lane module {module} is gone — a lane renamed or removed is a deliberate edit of the pin"
        );
    }
    let roles_files: Vec<(String, String)> = production
        .iter()
        .filter(|(path, _)| path.starts_with("forge/src/roles/"))
        .cloned()
        .collect();
    assert!(
        roles_files.len() >= ROLES_SOURCE_FLOOR,
        "the sweep read {} files under forge/src/roles/ (floor {ROLES_SOURCE_FLOOR})",
        roles_files.len()
    );
    let lane_holders = token_holders(&roles_files, &RELEASE_OWNER_TOKENS);
    assert_eq!(
        lane_holders,
        BTreeSet::from([RELEASE_HANDLE_EXPORTER.to_string()]),
        "a file under forge/src/roles/ other than dev_ops.rs names a release-owner token: {lane_holders:?}. \
         The release capability belongs to the DevOps lane module alone — another lane naming it is a second \
         claim on release, and the trait port counts (it is what carries the handle to a publish command)"
    );

    let port = files_containing(&production, "dyn ForgeReleaseExecutor");
    if let Some(entry) = production
        .iter()
        .find(|(path, _)| path == "forge/src/engine/re_runtime.rs")
    {
        let target = repo_root().join("forge/src/engine/re_runtime.rs");
        let on_disk = source::read(&target);
        eprintln!(
            "DEBUG cwd={:?}\nDEBUG repo_root={:?}\nDEBUG target={:?}\nDEBUG canonical={:?}\nDEBUG raw_len={} processed_len={} has_fn={}",
            std::env::current_dir().ok(),
            repo_root(),
            target,
            std::fs::canonicalize(&target).ok(),
            on_disk.len(),
            entry.1.len(),
            on_disk.contains("shared_forge_runtime")
        );
    }
    assert_eq!(
        port,
        BTreeSet::from(
            RELEASE_PORT_HOLDERS
                .iter()
                .map(|path| path.to_string())
                .collect::<BTreeSet<String>>()
        ),
        "the release handle reaches the engine only as a trait object through {RELEASE_PORT_HOLDERS:?}; \
         found {port:?}. A new holder is a new route to a publish command"
    );

    // ── 5. THE DOOR: one caller, three switch readers, no door in the web tier, no deploy shell. ─────────

    let callers = deploy_path_callers(&production);
    assert_eq!(
        callers,
        BTreeSet::from([THE_ONE_DOOR.to_string()]),
        "the publish door is called from {callers:?} instead of solely from {THE_ONE_DOOR}. The door is the \
         one production file that may run the deploy path, and it is gated by FORGE_ALLOW_PUBLISH at the \
         point of use (TST-ARCH-BOUNDARY-011, fact 3) — a second caller is a publish outside that gate"
    );

    let switch_readers = files_containing(&production, "env::var(\"FORGE_ALLOW_PUBLISH\")");
    assert_eq!(
        switch_readers,
        BTreeSet::from(
            SWITCH_READERS
                .iter()
                .map(|path| path.to_string())
                .collect::<BTreeSet<String>>()
        ),
        "the publish switch is read by {switch_readers:?} instead of {SWITCH_READERS:?} — the door gates on \
         it, the composition root logs it, the worker forwards it to a child; a fourth reader is a new actor \
         in the release path"
    );
    let door_code = &production
        .iter()
        .find(|(path, _)| path == THE_ONE_DOOR)
        .unwrap_or_else(|| panic!("{THE_ONE_DOOR} must be in the sweep"))
        .1;
    assert!(
        door_code.contains("publish_switch_off("),
        "the door no longer reads its switch"
    );

    let web_files: Vec<(String, String)> = production
        .iter()
        .filter(|(path, _)| path.starts_with("web/"))
        .cloned()
        .collect();
    assert!(
        web_files.len() >= WEB_SOURCE_FLOOR,
        "the sweep read {} files under web/ (floor {WEB_SOURCE_FLOOR})",
        web_files.len()
    );
    let web_holders = token_holders(&web_files, &RELEASE_OWNER_TOKENS);
    assert!(
        web_holders.is_empty(),
        "the web tier — the TECH cockpit included — names a release token: {web_holders:?}. It reads the \
         build stamp and the diagnostics; it must not hold a door to release or deploy"
    );

    let deploy_named = files_containing(&production, "\"deploy\"");
    assert_eq!(
        deploy_named,
        BTreeSet::from(
            DEPLOY_LITERAL_READERS
                .iter()
                .map(|path| path.to_string())
                .collect::<BTreeSet<String>>()
        ),
        "the production files naming the literal \"deploy\" changed: {deploy_named:?}. One owner \
         ({RELEASE_HANDLE_EXPORTER}) and three engine readers are pinned; a fifth is a new place the deploy \
         node is acted on, and it must be read before it is allowed"
    );

    // The shells. Publication is a Forge-owned code path, so NO profile may run a deploy command — the
    // DevOps agent included: it analyses release state and cannot fire a deployment.
    assert_eq!(
        agents::V2_AGENT_PROFILES.len(),
        11,
        "the agent roster changed; the deny below is asserted for every profile"
    );
    for profile in agents::V2_AGENT_PROFILES.iter() {
        let permissions = agents::v2_agent_permissions(profile, false);
        for pattern in DEPLOY_SHELL_DENIES {
            assert!(
                shell_denies(&permissions, pattern),
                "'{}' may run `{pattern}` on bash. Publication and deployment are Forge-owned code paths, \
                 never a model's shell: deploying is a Captain's production action \
                 (`scripts/deploy-prod.sh`) and publishing runs behind FORGE_ALLOW_PUBLISH in the door",
                profile.id
            );
        }
    }
    let devops_profile = agents::V2_AGENT_PROFILES
        .iter()
        .find(|profile| profile.id == agents::AGENT_DEVOPS)
        .expect("the DevOps agent exists");
    assert!(
        shell_denies(
            &agents::v2_agent_permissions(devops_profile, false),
            "vercel*"
        ),
        "the DevOps agent itself must be denied `vercel*`: DEV_OPS owns the release nodes and the release \
         evidence, not a deploy shell"
    );
}

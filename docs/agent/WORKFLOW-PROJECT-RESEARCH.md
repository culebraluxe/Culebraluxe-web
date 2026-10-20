# Workflow as its own project — research (2026-10-19)

Question: how do we put the workflow engine in its own git project while the app and Forge keep their
host layers, and while one Neon database / one `schema_migration` ledger stays shared?

Inputs: `WORKFLOW-EXTRACTION-PLAN.md` (approved direction), `W-1-DESIGN.md` (current split),
`APP-CODE-REVIEW-2026-10-19.md`, Cargo docs, release-plz docs.

## 1. What Cargo actually supports (and does not)

Cargo dependency sources are: crates.io/other registries, **git repositories**, and **local paths**.
A Cargo *workspace* is local: members must be directories in one repo, so "workflow as its own project"
means either a **git dependency** or a **registry dependency** for the hosts. ([Cargo: specifying
dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html))

Git dependency mechanics that matter here:

- `workflow-core = { git = "https://github.com/culebraluxe/workflow", tag = "v0.1.0" }` — Cargo fetches
  the repo, finds the crate anywhere in the tree, and **pins the commit in `Cargo.lock`**; it only
  moves when `cargo update` runs.
- `rev = "<sha>"` gives exact-commit pinning; `branch` tracks a branch (avoid for production).
- `version = "0.1"` alongside `git` only *checks* the fetched crate's version; it does not select the
  commit, and crates.io refuses published packages that depend on non-registry sources (except
  dev-dependencies).
- Private repos use Cargo's git authentication (credential helper/token); CI needs read access.
- Git dependencies download the **whole repository**, so keep the workflow repo small
  ([cargo#14689](https://github.com/rust-lang/cargo/issues/14689)).
- Submodules are fetched recursively; avoid them for the engine.

## 2. Layout options

| Option | Shape | Pros | Cons |
|---|---|---|---|
| **A. One workspace, separate crates** (today) | `workflow-core` + `workflow-neon` as workspace members of the app repo | zero versioning, one CI, refactors easy | not its own project; app release still couples engine |
| **B. New repo + git tags** (recommended first) | `workflow` repo: `workflow-core`, `workflow-neon`, conformance, definitions fixtures, migrations; hosts depend by `git` + `tag` | own history/CI/ForgeVCS; exact pins; no registry infrastructure | two-repo PR choreography; repo download; tag discipline |
| **C. New repo + private registry** | publish `workflow-core`/`workflow-neon` to a private registry (Cloudsmith/Artifactory/crates.io private) | semver resolution, smaller downloads, standard `cargo publish` | registry account/ops, publishing credentials, yanked-version handling |
| **D. Submodule / vendored copy** | git submodule or `vendor/` | simple checkout | version drift, duplicated CI, submodule pain; rejected |

Cargo cannot treat ForgeVCS as a dependency source: ForgeVCS is a Scion service, **not a Git host and
not a Cargo registry**. Its repo is the VCS home; Cargo still needs GitHub (or a registry). That is
compatible with the plan: ForgeVCS hosts the workflow repo for source control, while dependency
consumption uses tags from the Git remote.

## 3. Recommended shape

```text
workflow (new repo, later mirrored into ForgeVCS)
  Cargo.toml                 # workspace
  crates/workflow-core       # zero-dependency engine/kernel (today: workflow-core)
  crates/workflow-neon       # Neon/Postgres adapter (today: workflow-neon)
  crates/workflow-conformance# engine/store conformance suite + fixtures
  migrations/                # workflow-owned tables (applied by the shared applier)
  definitions/               # engine schema/fixtures only; hosts keep RE/FORGE XML
  RELEASE.md                 # tag/bump policy

app repo
  re-host/                   # app RE host (XML, runtime, facts, commands, port, worker)
  db + db-tool               # ONE migration applier/ledger for the shared Neon DB
  workflow-core/-neon = { git = "...", tag = "v0.x.y" }

forge repo (later)
  SDLC host + queue/roles/harness
  workflow-core/-neon = { git = "...", tag = "v0.x.y" }
```

Versioning: semver-ish tags `v0.MINOR.PATCH` for the workspace; hosts pin a tag, but `Cargo.lock`
pins the commit. Bump choreography: tag in workflow repo → PR in app/Forge bumping the tag → both
hosts' smoke/conformance suites must pass. `release-plz` automates version bumps/changelogs/tags and
explicitly supports **detached HEAD/colocated jj** repos, which matters for our lanes
([release-plz release](https://release-plz.dev/docs/usage/release)).

## 4. Preserving the cross-benefit (Forge fixes the app)

- `workflow-conformance` is the contract gate: engine semantics, definition parsing, store contract,
  timers/joins/tokens. Both hosts run it against the same pinned version.
- Host smoke tests: app RE host end-to-end; Forge SDLC role/job path. A core change that breaks a host
  fails the workflow repo's matrix (test both host fixtures) before a tag is cut.
- Never fork the engine. Hosts may add ports/handlers, not copy engine code.
- One bump PR per host with the tag; no floating `branch` in production manifests.

## 5. Neon / migrations (no DB fork)

- **One Neon DB, one schema, one `schema_migration` ledger, one applier** stays the rule.
- Workflow repo owns the **SQL for workflow tables** (`process_instances`, `tokens`, `tasks`, `jobs`,
  workflow definitions/versions) and ships it in its release artifact; the app repo's `db-tool`
  remains the single applier through the transition. Later, ownership can move with the ledger, but
  still exactly one writer.
- Ordering: the chain is interleaved today (44% of migrations touch Forge/workflow tables), so the
  workflow repo must not number its own independent chain against the same ledger. Use a
  prefix/namespace + an ordered manifest consumed by the one applier, or keep the files in the app
  repo until the ledger can move wholesale.
- `jobs` exists in DEV/PROD but no migration creates it: the workflow project must add an idempotent
  baseline migration (or a documented externally-provisioned check) before any fresh-DB replay.
- Expand/contract changes only; no destructive rename in one release.

## 6. Costs / risks

| Risk | Mitigation |
|---|---|
| Two-repo PR drift | tag bump PRs + conformance/smoke gate; `cargo update -p workflow-core --precise` |
| Git dep downloads full repo | keep repo small; split conformance fixtures if needed; registry later |
| Private repo auth in CI | deploy key/read token for workflow repo |
| jj detached HEAD/tagging | release-plz supports detached HEAD; tag from a clean `jj new <rev>` working copy |
| Migration double-apply | one applier/ledger, ordered manifest, no second runner |
| ForgeVCS not a Cargo source | keep GitHub/registry as dependency source; ForgeVCS mirrors source control |
| Host API churn | contract tests both hosts before tag; semver + deprecation window |

## 7. Revised extraction steps (after W-1's in-repo split)

1. Finish W-1: `re-host` out of forge; `workflow-core`/`workflow-neon` boundary green.
2. Create the `workflow` repo with the two crates + conformance + tag `v0.1.0`; CI runs conformance
   and a host-fixture matrix.
3. In app/Forge: replace path deps with `git` + `tag`; keep `Cargo.lock` pinned; run host smoke tests.
4. Move workflow migration SQL ownership behind the one applier (namespace/ordered manifest); add the
   `jobs` baseline; prove fresh-DB replay.
5. Mirror the repo into ForgeVCS (WIRE-1..); ForgeVCS is source-control home, not the Cargo source.
6. Revisit a private registry only if git-dep download/CI costs or semver resolution become a problem.

## Sources

- Cargo Book, "Specifying Dependencies" — git/path/registry, rev/tag, lockfile pinning, private auth,
  submodules: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
- Cargo issue #14689 — git dependencies fetch the full repository:
  https://github.com/rust-lang/cargo/issues/14689
- release-plz, `release` — tags/versions/changelogs, detached HEAD and jj support:
  https://release-plz.dev/docs/usage/release
- Cargo registries (alternative/private registries):
  https://doc.rust-lang.org/cargo/reference/registries.html

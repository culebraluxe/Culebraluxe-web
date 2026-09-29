-- Seven stories, one job: the tests the retired TypeScript suite used to hold, as Rust.
--
-- THIS row is what the Rust engine reads: rust/forge/src/engine/packet.rs:19-60 loads goal,
-- architect_brief, acceptance_criteria and assay_commands from storyboard_story. The markdown packet under
-- docs/agent/packets/ is for a human reviewer; for this batch the row IS the brief, deliberately, so the
-- enqueue stays cheap and one writer owns each file.
--
-- Why these seven: docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md findings A and B measured that every `guard:`
-- path AGENTS.md names is dead (workflow_app/ is gone) and that no Rust test stands where those guards stood;
-- its finding 6 and DEAD-COMMANDS.md:120 name the two stranded proofs. A guard with no file is a rule with no
-- enforcement, which is the state the "Never" list is in today.
--
-- Idempotent on id: re-running refreshes the brief without re-dispatching, because the Ready trigger fires on
-- insert or on a *change* of status (db/migrations/025_agent_work_queue.sql:104).
--
--   ./rust/target/debug/cli db-tool apply db/loads/stories_rust_tests_2026_09_29.sql prod     (from repo root)

begin;

-- 1 — the defect that killed engine run #2 after 50 minutes and nine role turns.
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-POOL-IO-01', 'HARDEN', 'An engine run survives a dropped database connection', 'High', 'Ready',

    $goal$A run survives the database connection it is using being taken away mid-run — the server closing an idle session, an idle-in-transaction kill (SQLSTATE 25P03), or a socket the peer has closed — and a failure that reaches a run is announced as an app_error row rather than ending it in one line of output.$goal$,

    $scope$In: the classification/retry seam in rust/core/db (a pure, tested "is this failure transport-level, and did the transaction survive it" predicate plus the one retry), and the shape of the transaction the engine holds across a role turn (rust/core/workflow `workflow.step` / rust/forge/src/engine) — a transaction must not be open while an external role turn runs for minutes, or the fix is to stop holding it that way rather than to lengthen a timeout. Out: no schema change, no migration, no change to the HTTP server's pool sizing, and NO blanket test_before_acquire(true) — AGENTS.md records that ping-on-every-checkout as a measured 80ms-per-query regression, so it stays off.$scope$,

    $ac$1. cargo test -p db passes and includes a test for the classification predicate: an IO/transport error (Broken pipe, connection reset, unexpected EOF) and SQLSTATE 25P03 are retryable; a SQLSTATE error that means the work was rejected (a constraint violation, a syntax error, a unique violation) is not.
2. The retry runs the operation again on a NEW connection, not the same one, and retries at most once.
3. A retried failure still announces itself through db::capture (an app_error row, operation named), so a run that limped through is visible in the rows rather than silent.
4. NO TRANSACTION IS OPEN ACROSS A ROLE TURN. This is the load-bearing criterion, because it is the measured cause: report where the transaction was opened and what now bounds it, and if the answer is "a transaction is still open across the turn", say so instead of shipping a longer timeout.
5. cargo check --workspace --all-targets and cargo test -p db -p forge are green.$ac$,

    $brief$EVIDENCE (measured 2026-09-29, scheduled runs against PROD, `ENG-AUTH-GOOGLE-01`). Two symptoms, and the second one names the cause:

    error communicating with database: Broken pipe (os error 32)            (a 50-minute run, 9 role turns)
    Unknown during workflow.step (incident d42d33c2-20cc-4697-81b1-86a02cbc5e0b, sqlstate 25P03):
      terminating connection due to idle-in-transaction timeout             (a run that died 2m45s in, no role turn output)

SQLSTATE 25P03 is `idle_in_transaction_session_timeout`: the server ended the session because a transaction had been idle too long. A session sitting idle between statements is not killed by that — only one that is *inside* an open transaction is. So the engine holds a transaction open while the role runner drives `opencode` for minutes, and the server takes the session away underneath it. That also explains the Broken pipe: a socket whose server side has gone, written to by the next statement, is exactly EPIPE. Assume one cause until someone proves two.

WHERE IT DIES. `Unknown during workflow.step` — the step boundary, i.e. the runtime's transaction (rust/core/workflow, the store's `TxStore`), with the role turn running inside it.

WHAT IS ALREADY THERE. The pool probes a connection that has sat idle (`FORGE_DB_IDLE_PROBE_MS`, 30s, rust/core/db/src/pool.rs:135-146) and discards one whose probe fails, and `spawn_keepalive` (pool.rs:227) keeps a pool warm — but its only caller is the HTTP server (rust/server/src/http_runtime.rs:22). Neither helps a session killed *mid-transaction*; the probe only runs at checkout, and the keepalive holds no transaction.

THE ENGINE'S BUDGETS ARE NOT THE ANSWER HERE. `rust/forge/src/engine/db_budget.rs` already raised the engine's own statement ceiling and connect budget (commits 59f75f8c, 9a1d53d7) — that fixed a different two failures (a >30s first statement, and a 12-second tick that claimed nothing). Lengthening `idle_in_transaction_session_timeout` would buy hours and still lose the work when it fired, and it would weaken the one guard that stops a leaked transaction from holding locks across the control plane. Prefer not holding the transaction.

RISKS. (1) A retry can double an operation whose first attempt actually landed. Restrict it to transport failures, keep it to one attempt, and read the operation before wrapping it: a transaction that died mid-flight has taken its writes with it, so re-running the step must be safe or must be made safe. (2) Do not let the retry swallow a failure — criterion 3 exists so a sick database cannot go quiet. (3) This seam is shared with the server, whose request path must not gain latency (see Out). (4) If the transaction is opened by the harness adapter rather than the runtime, say so; the criterion is about the effect, not about which crate gets the blame.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p db
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

-- 2 — AGENTS.md:162 "Never run Forge against DEV", whose guard file no longer exists.
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-GUARD-ENV-RUST-01', 'HARDEN', 'The environment-routing rule becomes a Rust test', 'High', 'Ready',

    $goal$The rule AGENTS.md states as "Forge runs execute against PROD only, and a production process must never silently fall back to DEV" is enforced by a Rust test in the crate that owns the resolution, so the rule survives the deletion of the TypeScript guard that used to hold it.$goal$,

    $scope$In: tests in rust/core/db (tests/ and/or a unit module) around the declared-target resolution the handbook names in rust/core/db/src/pool.rs (`resolve_declared_target`), asserting the resolution table and the fail-closed behaviour. Out: no change to the resolution itself unless a test uncovers a real hole, no schema, no engine change, and do not edit AGENTS.md here — the handbook is owned by ENG-GUARD-AGENTS-LINT-01 so two stories never write it at once.$scope$,

    $ac$1. A Rust test pins the resolution table: APP_ENV=production and APP_ENV=prod and VERCEL_ENV=production resolve to Prod; an unset APP_ENV resolves to Dev.
2. A Rust test pins behaviour on contradiction: a production marker combined with a development marker must not resolve to Dev silently. Assert which behaviour the code has today, and if that behaviour is "silently Dev", the test must not bless it — fix it, and say so in the report.
3. Each test carries a doc comment quoting the AGENTS.md:162 sentence it guards, so a reader who finds the test finds the rule.
4. cargo test -p db and cargo check --workspace --all-targets are green.
5. The report states the exact Rust test path: ENG-GUARD-AGENTS-LINT-01 has to point AGENTS.md's `guard:` line at it.$ac$,

    $brief$WHY. AGENTS.md:162 — "Never run Forge against DEV. Forge runs (engine lanes, dogfoods, splits, role attempts) execute against PROD only — the environment is not something a run may flip." Its enforcement is `guard: workflow_app/tests/db-routing.test.ts`. `workflow_app/` does not exist in this repository and the file does not survive under `legacy/` either (docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md finding A: every guard path the handbook names is dead). The rule is a sentence with nothing behind it.

WHY THIS ONE FIRST. Its failure is silent and destructive. `resolve_declared_target` decides whether a process talks to the production database, and AGENTS.md states the mechanism plainly: "a production deploy is a production database connection — there is no dry run and no separate switch."

WHERE THE CODE IS. rust/core/db/src/pool.rs — `Database::connect_from_env` (line 50), `Database::connect_target` (line 58), and the declared-target resolution. `VERCEL_ENV=production`, `APP_ENV=production`, `APP_ENV=prod` are the production markers; the boot line and `GET /v1/diagnostics/db` report which target a live process is on (`target=dev` / `target=prod`).

RISKS. (1) Do not depend on the ambient environment of the machine running the test — cargo test runs many tests in one process, so set explicitly and restore, or prefer a pure function of the markers. (2) Do not test by connecting to a database; the resolution is a pure decision and pinning it is the whole point. (3) If there is no seam that can be called with explicit markers, that discovery IS the deliverable: report it and add the smallest seam that lets the rule be pinned.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p db
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

-- 3 — AGENTS.md:160/161/164 "no git in QA", "a worker never pushes", and :133 "never commit secrets":
--     three guard files in a directory that no longer exists.
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-GUARD-FORGE-RUST-01', 'HARDEN', 'The engine-side guards become Rust tests', 'High', 'Ready',

    $goal$Three rules the engine must obey — a QA verdict never turns on a git sha, a worker never pushes or rebases, and a publish path never lets a secret through — are enforced by Rust tests in rust/forge, where the code that must obey them lives.$goal$,

    $scope$In: Rust tests (rust/forge/tests/, one file per rule or one guarded file with named tests) that fail when the engine regains the prohibited behaviour. Out: no engine behaviour change unless a test finds the rule already broken, no schema, no UI, and do not edit AGENTS.md (owned by ENG-GUARD-AGENTS-LINT-01).$scope$,

    $ac$1. A test fails if the QA/Assay release path consults a git sha or a commit identity — the rule AGENTS.md defends at line 160 as "PAID CODE > GIT SHA", because on 2026-09-19 a QA gate that demanded a sha had to be deleted the same day for voiding work that was already paid for.
2. A test fails if any code path reachable from a worker role pushes, merges or rebases (AGENTS.md:161) — name the seam it watches.
3. A test fails if the publish scan stops covering a path it covered before, or if a known secret shape (a DATABASE_URL with a password, a bearer token, a private key header, an .env.local body) passes the scan. The rule is AGENTS.md:133 — "Commit secrets or .env.local".
4. Each test carries a doc comment quoting the handbook sentence it guards.
5. cargo test -p forge, cargo check --workspace --all-targets green; the report names each test's path and says plainly which of the three rules turned out to be already broken if any did.$ac$,

    $brief$WHY. Three `guard:` lines in AGENTS.md point at files in `workflow_app/tests/`, a directory that does not exist: `forge-qa-no-git.test.ts` (cited twice, lines 160 and 164), `worker-commit-identity.test.ts` (161), `publish-scan-coverage.test.ts` (133). Measured in docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md finding A, with no surviving copy under `legacy/`. These three rules are about the engine's own behaviour, so rust/forge is where a test can hold them.

WHAT EACH RULE COSTS WHEN IT BREAKS. Line 160's rule exists because a review once demanded QA "measure the code the route identifies" — the git-bearing check voided a release that had already been paid for, and a sha guard had to be removed the same day (ENG-FORGE-QA-NO-GIT-GUARD-01, `docs/agent/MEMORY.md`). Line 161's rule exists because a worker that pushes collides with every other lane in a one-branch house. Line 133's rule is the boundary that keeps credentials out of a production-sensitive `main`.

HOW TO HOLD A RULE THAT IS ABOUT CODE SHAPE. Two honest options, and the story accepts either: (a) pin the behaviour — drive the function with git absent and assert it still returns the same verdict; or (b) a source scan in the shape of the gates already in this repository (`rust/cli/src/forge/ts_sweep.rs` reads files and fails; `rust/cli/src/forge/vendor_block.rs:168` walks guardrail blocks). Do not invent a third mechanism. A scan must be scoped to the files it names, print what it matched, and fail loudly — a scan that passes because it read nothing is the failure mode this story exists to avoid.

RISKS. (1) A shape test that greps for the token `rev-parse` can be defeated by a rename, and a behaviour test can be defeated by an early return — say which one you wrote and what its blind spot is. (2) rust/forge/tests/ already holds `forge_runtime.rs`; do not fold these into it, they answer different questions. (3) If the right seam turns out to live in `rust/core/workflow` (the engine) rather than `rust/forge` (the SDLC process), put the test where the code is and say so — the location is not the deliverable, the enforcement is.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p forge
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

-- 4 — AGENTS.md:151 "NO TREES. EVER.", :172 "one fact has ONE writer", :166 "WhatsApp is not an identity type".
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-GUARD-REPO-RUST-01', 'HARDEN', 'The repo-wide guards become Rust tests', 'High', 'Ready',

    $goal$Three rules that are about the whole repository rather than one crate — no per-lane worktree may come back, one fact has one writer, and WhatsApp is not a new identity type — are enforced by Rust tests that read the tree and fail.$goal$,

    $scope$In: Rust tests in rust/cli (beside the scans that already live there: rust/cli/src/forge/ts_sweep.rs, rust/cli/src/forge/vendor_block.rs:168) or rust/core/domain for the identity rule, whichever crate owns the fact being guarded. Out: no behaviour change to the engine or the domain beyond what a failing test forces, no schema, and do not edit AGENTS.md (owned by ENG-GUARD-AGENTS-LINT-01).$scope$,

    $ac$1. no-tree-residue (AGENTS.md:151): a test fails if a tracked file introduces a per-lane worktree — the word for it in a path or a command, a writer that emits it, an npm/pnpm script that creates one. The rule's history is the reason: 83 worktrees under Documents/Culebraluxe-worktrees plus .assay-workspaces/, all deleted, and a verdict about a tree instead of about the code. "NO TREES. EVER."
2. one-writer-per-column (AGENTS.md:172): a test fails if two code paths in the workspace write the same column of a canonical table — the audit the deleted `column-writer-audit.test.ts` performed. If a source-only scan cannot decide that, say so in the report and pin the narrower thing that IS decidable (for example: the set of files that write a named table/column is a list the test holds, and a new file joining that set fails the test).
3. WhatsApp is not an identity type (AGENTS.md:166): a test fails if WhatsApp is introduced as a new identity kind — the identity enum/registry stays unchanged, and a WhatsApp number is attributed through the existing mechanism. Read rust/core/domain (and db/src/whatsapp.rs) before deciding the shape of the assertion.
4. Each test carries a doc comment quoting the handbook sentence it guards, prints what it matched, and fails when it reads nothing at all (an empty scan is a failure, not a pass).
5. cargo test -p cli (and -p domain if the identity test lands there), cargo check --workspace --all-targets green.$ac$,

    $brief$WHY. AGENTS.md:151, :166 and :172 name `workflow_app/tests/{no-tree-residue,whatsapp-attribution,column-writer-audit}.test.ts` as their guards. `workflow_app/` does not exist and `legacy/workflow_app/tests/` contains only `fixtures/` — measured in docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md finding A. Every one of these rules has already been broken once in this repository's history, which is why the handbook states them in capitals.

WHAT THE THREE COST, FROM THE RECORD. no-tree-residue: the estate grew to 83 worktrees plus `.assay-workspaces/`; on 2026-09-16 a lane produced verdicts about a tree rather than about the code, and every QA pass was void. one-writer-per-column: the Architect reply parser still stood beside the findings rows and the QA verdict had three authors, so "both produced verdicts nobody could trust, and both were ours, not a model's". WhatsApp: treated as a new identity type it would fragment attribution, which is the one thing the intake path must not do.

THE PRECEDENT TO FOLLOW. This repository's Rust scans are the shape to copy: `rust/cli/src/forge/ts_sweep.rs` (walks the two roots, counts four classes, fails on drift) and the guardrail walk in `rust/cli/src/forge/vendor_block.rs:168`. They are tests-as-gates: deterministic, sorted output so two runs can be diffed, and they fail when the tree and the claim disagree.

RISKS. (1) False positives are worse than a missing guard: a scan that fails on a legitimate file teaches people to bypass it. Scope each scan to the paths it names and let the rest of the tree be. (2) A scan that reads zero files must FAIL — that is how the broken-TS sweep keeps itself honest, and how this one stays honest. (3) The identity rule must not be enforced by inventing a new enum variant to test against; read the existing identity mechanism first.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p cli
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

-- 5 — the missing lint behind finding A: nothing checks that a `guard:` path exists.
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-GUARD-AGENTS-LINT-01', 'HARDEN', 'AGENTS.md guard paths are linted, not assumed', 'High', 'Ready',

    $goal$A Rust gate fails when AGENTS.md names a guard path that does not exist, so the handbook cannot drift from the tree it describes — the failure that let all eight `workflow_app/tests/*.test.ts` guard lines survive the deletion of `workflow_app/` unnoticed.$goal$,

    $scope$In: a Rust gate beside the existing handbook gates in rust/cli/src/forge (vendor_block.rs and its `orphaned_guardrails` walk is the nearest precedent), a test for it, and the AGENTS.md corrections that the gate then proves — every `guard:` line points at a real file (the Rust tests the sibling guard stories land; a line whose rule genuinely has no automated check keeps AGENTS.md's own wording, "guard: NONE — <why>"), plus finding B: the nine deleted TypeScript paths in the handbook are corrected to their Rust equivalents, and the Error Capture Obligation's retired seams are rewritten to the Rust seams (db::capture, the panic hook, ApiError::into_response). Out: no engine change, no schema, no change to any rule's meaning.$scope$,

    $ac$1. The gate fails when a `guard:` line in AGENTS.md names a path that does not exist, printing the line number and the path. It is deliberately satisfied by AGENTS.md's own "guard: NONE — <reason>" form, so an unenforceable rule is a written decision rather than a silent hole.
2. The gate fails when a guard path resolves to a file with no test in it (an empty file is not a guard) — or, if that is not decidable, say so in the report and gate only existence.
3. The gate runs in CI: `cargo test --workspace --all-targets` (`.github/workflows/gates.yml:333`) executes it, or the CLI command is added to that workflow. A guard that must be remembered by hand is not a guard.
4. AGENTS.md as corrected passes the gate, and the diff of AGENTS.md is only: guard paths pointed at real files, "guard: NONE — reason" where none exists, the dead TypeScript paths replaced with their Rust equivalents, and finding B's Error Capture Obligation seam list. No rule changes meaning.
5. cargo test -p cli green; the report pastes the gate's own output on the corrected handbook.$ac$,

    $brief$WHY. docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md finding A: "Every guardrail path in AGENTS.md is dead — the 'Never' list has no enforcement." Eight `guard:` lines (AGENTS.md:133, 151, 160, 161, 162, 164, 166, 172) all point into `workflow_app/tests/`, deleted with the TypeScript application; `legacy/workflow_app/tests/` holds only `fixtures/`. The handbook reads as if nine rules were guarded. Nothing checks a path, so nothing noticed — and the same absence is why finding B (nine deleted TypeScript paths still called canonical) survived as long as it did.

WHAT THIS GATE IS NOT. It is not a test of the rules; it is a test of the CLAIM that the rules are tested. That distinction is the whole value: a path that exists but holds no test still passes a naive gate, which is why criterion 2 asks for more than existence if that is decidable.

THE PRECEDENT. rust/cli/src/forge/vendor_block.rs renders the four load-bearing guardrails into vendor pointer files and `pnpm forge:packet-lint` fails "when a block drifts from a fresh render or when a backing sentence disappears from this file". The same walk already knows about `orphaned_guardrails` (vendor_block.rs:168) — this gate is the guard-path half of that idea.

RISKS. (1) The corrected handbook must not weaken a rule to make the gate pass: "guard: NONE — reason" is for a rule that genuinely cannot be automated (AGENTS.md uses it today for the two destructive-database rules), never for a rule whose test was simply not written yet. If you find yourself reaching for it, say so in the report and leave the line failing. (2) The sibling stories (ENG-GUARD-ENV-RUST-01, ENG-GUARD-FORGE-RUST-01, ENG-GUARD-REPO-RUST-01) create the files this story points at; if they have not landed, point the line at nothing and FAIL rather than pointing it at a plausible-looking path. A wrong path is worse than a missing one, because it looks enforced. (3) AGENTS.md is a protected file with its own gate (`scripts/protected-files.test.ts`, per docs/agent/HANDOFF-forge-port-2026-09-28.md) — run that gate in the assay and expect it to fire; report it rather than working around it.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p cli
cargo run --manifest-path rust/Cargo.toml -p cli -- forge guard-lint$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

-- 6 — finding 6: the parity ledger says it is generated, and its generator is gone.
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-PARITY-LEDGER-01', 'HARDEN', 'docs/rust-parity-ledger.md is generated again, in Rust', 'High', 'Ready',

    $goal$The parity ledger's generator exists in Rust, so the one document that answers "which service serves this in production?" can be regenerated instead of hand-edited — and it refuses to run when the map and the repository disagree.$goal$,

    $scope$In: a Rust command in rust/cli (the `forge` family) that reads scripts/rust-parity-map.json and the mounted router, and renders docs/rust-parity-ledger.md; a test; and the ledger regenerated by it so the committed file and the command agree. Out: no change to what the ledger claims, no change to the router, no schema, no new capability rows invented — if a row cannot be derived, say so rather than guessing.$scope$,

    $ac$1. `forge parity-ledger` (or the name you choose) prints the ledger and `--write` regenerates docs/rust-parity-ledger.md; a `--check` mode exits non-zero when the committed file differs from a fresh render.
2. Running it twice in a row leaves the worktree clean the second time (a first run may legitimately rewrite the file once, which is the regeneration this story exists for).
3. The header no longer names a Node command: it names the Rust command, and it still says not to hand-edit.
4. The existing test `rust/server/tests/signature_routes.rs` — which fails "if the router and this map disagree" — is still green, and the report shows the ledger's route count before and after.
5. cargo test -p cli -p server, cargo check --workspace --all-targets green.$ac$,

    $brief$WHY. docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md finding 6: "the parity ledger can no longer be regenerated (measured)". The ledger's own header says:

    Generated by `node --import tsx scripts/rust-parity-ledger.ts` — do not hand-edit. The run FAILS if the map and the repository disagree, so a rename or a new route forces this file to be regenerated instead of drifting.

That script does not exist in this repository. So the file is hand-maintained while it claims to be machine-generated, and the claim "the run FAILS" is false: there is no run. What still holds is the downstream guard — `rust/server/tests/signature_routes.rs:1-5` asserts the Signature capability's row against the router — so the ledger is half-guarded: one row is checked, the rest is on trust.

WHAT THE LEDGER IS FOR. `docs/rust-prod-checklist.md` calls it the thing that answers the first question in an incident ("which service answers this?"), and the map behind it (`scripts/rust-parity-map.json`) carries the `rustStatus` / `productionPath` distinction that the port's honesty depends on: "a port can be complete and still cut over to nothing" (docs/rust-parity-ledger.md:8-9).

HOW TO READ THE MAP. scripts/rust-parity-map.json is the input; the router is the other one. The rendered numbers in the committed ledger ("113 routes mounted (read from the router, not from this file)", "13 built · 1 partial · 0 not started") are the checksum: if your generator reproduces them on this tree, it is reading the same things.

RISKS. (1) The temptation is to render a SIMPLER ledger than the one committed and call it regenerated — that silently deletes rows (the `firms` / `signature` / `whatsapp-intake` notes are load-bearing prose). Diff before and after and account for every changed line. (2) Route extraction from the router is the hard half; do not fake it with a hard-coded count. If the routes must come from a source other than the router, say exactly which and why. (3) Do not edit docs/agent/MEMORY.md or any handbook in this story.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p cli
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

-- 7 — DEAD-COMMANDS.md:120: "PORT as a Rust test. ... only its coexistence proof is stranded."
insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-WHATSAPP-COEXISTENCE-RUST-01', 'HARDEN', 'The WhatsApp coexistence proof is a Rust test', 'Medium', 'Ready',

    $goal$The coexistence behaviour of the WhatsApp intake — Meta coexistence events and the message path must not double-count or lose an interaction — is proven by a Rust test, because the TypeScript test that proved it was deleted while the implementation it proved is alive in Rust.$goal$,

    $scope$In: a Rust test (or a small set) over the Rust implementation: rust/core/db/src/whatsapp.rs, rust/integrations (the Meta payload verification/normalisation), rust/forge/src/engine/facts.rs or whatever crate holds the coercion, and the domain attribution. Out: no new WhatsApp behaviour, no schema unless a test proves a column is missing (then say so and stop), no change to rust/ui, and do not edit AGENTS.md.$scope$,

    $ac$1. A Rust test proves the coexistence property that was stranded: two sources describing the same WhatsApp conversation (a coexistence event and a normal inbound message) produce ONE interaction, not two, and the identity is attributed through the existing mechanism rather than a WhatsApp-specific identity type (AGENTS.md:166).
2. The test names the retired proof it replaces (`whatsapp-coexistence-completion.test.ts`, per docs/agent/DEAD-COMMANDS.md:120) in its doc comment, so the next reader can find the history.
3. If the property turns out NOT to hold in the Rust implementation, that is the finding: report it with the failing assertion and stop — do not fix it inside this story.
4. cargo test for the crate(s) touched and cargo check --workspace --all-targets green.
5. The report states which crate the test landed in, and why that crate owns the fact.$ac$,

    $brief$WHY. docs/agent/DEAD-COMMANDS.md:120 lists `whatsapp:coexistence:test` → `whatsapp-coexistence-completion.test.ts` with the verdict: "**PORT as a Rust test.** The implementation is not lost (integrations + `db/src/whatsapp.rs` + screens all exist); only its coexistence proof is stranded". The TypeScript file is deleted (verified: no tracked file matches its name outside nothing at all). The behaviour it proved is still shipped.

WHAT IS SHIPPED TODAY. The parity ledger rows it: `whatsapp-intake` is "partial" — "Rust verifies and normalises Meta payloads and the mounted webhook route is the Rust one. rustStatus stays 'partial' because durable inbox/ODS persistence is not finished" (docs/rust-parity-ledger.md:33). So this story does not finish that capability; it re-establishes the proof that what IS shipped behaves, which is the cheapest honest thing the deletion took away.

RISKS. (1) Coexistence semantics are Meta-specific: read the integration's own normalisation before writing an assertion, and if the rule is "the same conversation arriving twice is one interaction", say which key decides sameness (a provider id, a phone number, a thread id) instead of assuming. (2) A test that asserts a helper returns what the helper returns proves nothing: drive it through the intake path the webhook uses. (3) Do not widen this into the unfinished durable-persistence half — that is a different story with a schema.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p db
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream = excluded.workstream, title = excluded.title, priority = excluded.priority,
    goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
    architect_brief = excluded.architect_brief, assay_commands = excluded.assay_commands, updated_at = now();

commit;

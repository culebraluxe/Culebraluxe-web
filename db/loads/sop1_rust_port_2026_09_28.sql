-- SOP1 (Pippin Forge Watch SOP) — the 2026-09-28 amendment for the Rust port. The doctrine is unchanged; the commands and
-- mechanics beneath it moved from TypeScript to Rust, and each fact below was checked against the code on this date.
-- Appends once (guarded by the amendment's own heading), so it is safe to run again.
--
--   ./rust/target/debug/cli db-tool apply db/loads/sop1_rust_port_2026_09_28.sql dev      (from the repo root)
--   ./rust/target/debug/cli db-tool apply db/loads/sop1_rust_port_2026_09_28.sql prod

update storyboard_story
set architect_brief = architect_brief || $sop$

================================================================================
RUST PORT AMENDMENT 2026-09-28 - THE SAME DOCTRINE, NEW MACHINERY
================================================================================

The doctrine and the 2026-09-11/12 amendment above still hold. Forge is now Rust (rust/forge), and every
TypeScript script the earlier amendment names is dead. What a watcher uses today:

1. WHAT RUNS FORGE. launchd com.culebraluxe.agent-worker ticks every 180s (installed by
pnpm agent:scheduler:install, stopped by pnpm agent:scheduler:stop) and runs scripts/agent-worker-once.sh, which
runs the Rust forge-worker binary (rust/forge/src/bin/forge_worker.rs). Stop the scheduler before any manual drive.

2. HOW TO WATCH - ASK THE ROWS. pnpm forge:doctor (open engine tasks, active claims), pnpm forge:batch:status and
pnpm forge:roi are Rust (cli forge ...) and read PROD. The rows: agent_work_item, forge_engine_task_execution,
forge_tool_artifact, storyboard_story_run, forge_workflow_evidence, app_error. One query, one answer - never a log
tail. pnpm forge:clean is a PRODUCTION action with --force: it needs the captain's go every time.

3. PROD ONLY, STILL. The fail-closed guard is rust/forge/src/engine/execution_target.rs
(FORGE_EXECUTION_ENVIRONMENT = "PROD"). pnpm forge:sync-history is DEAD (scripts/sync-forge-history.ts) and has no
Rust port: a history gap has no recovery tool today. Report it; do not re-run work to fill it.

4. RELEASE GATES. pnpm db:parity and pnpm db:migrations are now Rust (cli db-tool parity / status). Still gates,
not reports, and a branch reset still hides drift.

5. THE ARCHITECTURE GATE NEVER RUNS. rust/forge/src/engine/qa_adjudicate.rs reads arch_ran, and no Rust code sets
it, so the honest reading is INCOMPLETE on every run. pnpm forge:tools is dead. knip and dependency-cruiser are now
installed but check TypeScript, which is not the product.

6. DEPLOY RECEIPTS. Release evidence is now derived in Rust (derive_release_evidence,
rust/forge/src/engine/role_slice.rs). Whether real runs populate it is NOT VERIFIED - check the rows before
classifying a devops-receipt HOLD. A fabricated receipt is still worse than a HOLD.

7. THE SPLIT DOOR. The FORGE_SPLIT_* switches no longer exist. split_eligibility (engine/graph.rs) and the split
join (engine/split_join.rs) are ported, but only tests call split_eligibility, so a split fan-out is NOT VERIFIED as
wired into a live drive. Do not expect a story to occupy two slots.

8. DEAD COMMANDS STILL ON THE MENU. forge:sync-history, forge:tools, forge:decision and story:status point at dead
TypeScript and cannot run. docs/agent/MAP-engine.md is the current command table.$sop$,
    notes = notes || $sop$

[2026-09-28] RUST PORT AMENDMENT appended to the brief: same doctrine; the commands are now Rust (forge:doctor,
batch-status, roi, db:parity, db:migrations), forge:sync-history and forge:tools are dead with no port, the
architecture gate never runs (INCOMPLETE), and deploy receipts and the split door are not verified as live.$sop$,
    architect_brief_updated_at = now(),
    updated_at = now()
where id = 'SOP1'
  and rollup = false
  and position('RUST PORT AMENDMENT 2026-09-28' in coalesce(architect_brief, '')) = 0;

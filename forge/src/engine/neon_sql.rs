//! The one literal the engine shares with nobody else. The SQL is NOT here any more.
//!
//! WHY THE STATEMENTS LEFT (2026-09-29). This file held a second copy — never executed — of statements
//! `db::ForgeEngineDao` already owns: `workflow_command_receipt` (claim/finalize/read/watermark),
//! `forge_workflow_evidence` (read), `storyboard_story` (ledger read, repair/replan increments) and the
//! Story Packet read. Two copies of one fact with one of them dead is not a reference, it is a decoy: the
//! column-writer audit (`cli/src/forge/repo_guards.rs`) matched this file's increment text and named
//! it a writer of `forge_repair_attempts` / `forge_replan_attempts`, while the writer that runs is
//! `db/src/forge_engine.rs`. Deleting the bodies makes that fence report the truth. (Even this
//! paragraph must not spell that statement out: the audit matches source text, comments included.)
//! The statements are in git history (`git show ae16ef38:forge/src/engine/neon_sql.rs`) and were
//! copied from `db/workflow-command-receipt.ts` / `db/forge-workflow-evidence.ts` (now `legacy/`).
//!
//! What remains has no SQL body to live with a DAO: the receipt prefix that names the completion unit.
//! Do not invent columns. Never give a fact a second writer.

pub const RECEIPT_PREFIX: &str = "forge.completion:";

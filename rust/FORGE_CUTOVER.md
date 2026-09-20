# Forge + workflow cutover (aggressive)

Neon schema stays. TypeScript must stop being the execution path.

## Done in Rust
- workflow kernel (engine, tokens, tasks, jobs, expressions, NeonStore)
- FORGE_SDLC XML load/validate/deploy policy
- OpenCode harness
- QA adjudicate + assertion provenance
- release receipts / integration attestation
- PROD lane gate, split-join, holds, worktree provision
- self-heal loop, agent_work claim, workspace lineage
- decision injection + story scope
- ready gate, routing brain, turn budget, claim blocker, classify line

## Cut over now
1. Run stories with `cargo run -p forge --bin forge -- --story <id> --work-type FEATURE`
   (`APP_ENV=production`, `DATABASE_URL` set). This replaces `scripts/forge-engine-worker.ts`.
2. Stop dispatching `pnpm forge:engine`. Point package.json `forge:engine` at the Rust bin.
3. Keep TypeScript only as a compile-time façade until the Next actions are deleted.

## Remaining TS that still must die (next commits, same week)
- `workflow_engine/lib/workflow/engine.ts` — already ported; delete after rust host is default
- `workflow_app/forge/agent-runtime-role-runner.ts` — DeepSeek invoker body; do not port, delete after OpenCode-only
- observer / board-sync / morning-pack / night-driver / doctor / scorecard — operator tools
- `workflow_engine/app/actions/workflow.ts` — Next.js actions calling the TS engine; switch to rust HTTP or delete

## Rule
No new Forge behavior lands in TypeScript. Dual-write of reducer + engine is refused.

# ENG-FORGE-V4-10 — Worker Progress Telemetry

## Goal
Give the human operator clear live liveness while `pnpm agent:work` waits on an external worker, without changing orchestration semantics.

## Architect Brief
Add small, factual terminal progress telemetry to the durable `agent:work` path. While Smith or Assay is Running, periodically print a single concise status line so the operator can tell the worker is alive. Do not stream model chain-of-thought or provider internals; show only runtime facts Forge already knows.

Preferred operator line shape:

`Smith running · 3m 20s · heartbeat 8s ago · 10% · deepseek-harness`

Exact wording may vary, but it must remain compact and factual.

The telemetry should come from the existing worker lifecycle / heartbeat path, not a second polling worker or new scheduler. Reuse current runtime facts where possible.

## Scope
- `pnpm agent:work` / durable worker CLI only.
- Print periodic liveness while external execution is Running.
- Include role or position, elapsed time, latest known heartbeat age or equivalent factual liveness age, factual completion/step if available, and runtime adapter/profile when available.
- Avoid noisy rapid output; a modest interval is enough.
- Preserve existing terminal result output.
- Add focused tests for formatting / cadence behavior using injectable time or callbacks where practical.
- Keep implementation provider-neutral.

## Acceptance Criteria
1. A long-running Smith invocation visibly emits periodic progress after the initial `command:` / workspace lines.
2. A long-running Assay invocation gets the same behavior.
3. Progress output contains no fabricated percent or timestamps. If a fact is unavailable, omit it rather than invent it.
4. Telemetry does not expose model private reasoning, prompts, credentials, environment secrets, DB URLs, or raw provider session contents.
5. The progress mechanism does not create another worker, another control-plane claim, or another external model invocation.
6. Existing worker heartbeat persistence remains authoritative; telemetry is display-only.
7. Existing autonomous result output is unchanged except for the added progress lines while Running.
8. V3 Scout→Smith→Assay order is unchanged.
9. V4-08 execution-contract enforcement is unchanged.
10. V4-09 harness-owned candidate commit behavior remains unchanged.
11. No schema, migration, UI, provider activation, swarm/parallelism, or broad refactor.
12. Scoped tests pass.

## Context Refs
- `scripts/agent-work.ts`
- `agent-runtime/agent-runtime-adapter.ts`
- `agent-runtime/deepseek/deepseek-harness-adapter.ts`
- `agent-runtime/gateway/cli-agent-adapter.ts`
- `agent-runtime/types.ts`
- `agent-runtime/harness-owned-commit.test.ts`
- `docs/agent/packets/ENG-FORGE-V4-08.md`

## Test Mode
SCOPED

## Assay Commands
```bash
pnpm exec tsx --test agent-runtime/worker-progress.test.ts
```

```bash
pnpm exec tsx --test \
  agent-runtime/harness-owned-commit.test.ts \
  agent-runtime/readiness.test.ts \
  agent-runtime/gateway/cli-agent-adapter.test.ts \
  agent-runtime/gateway/provider.test.ts \
  agent-runtime/team.test.ts \
  agent-runtime/orchestrate.test.ts \
  agent-runtime/orchestrate-apply.test.ts \
  agent-runtime/repositories.assay.test.ts
```

## Skills
Small runtime/CLI observability change. Prefer one formatting/helper seam plus narrow integration over broad adapter changes.

## Loop
Architect packet → Smith → harness-owned candidate commit → Assay → accepted result → main.

## Out of Scope
- Streaming model thought/reasoning
- Streaming raw model stdout token-by-token
- New database tables or columns
- Web UI telemetry
- Automatic Architect execution
- Story decomposition
- Multiple Smiths
- Swarm / parallel execution
- Warp cloud execution
- OpenCode / Pi activation
- Provider roster changes
- Broad regression

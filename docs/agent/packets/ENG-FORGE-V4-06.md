# ENG-FORGE-V4-06 — Gateway Execution Safety Parity

## Goal

Give every CLI gateway executor the same fail-fast execution-target boundary already enforced by the DeepSeek native harness.

## Invariants

- Gateway child processes run in the isolated worker worktree when present.
- The intended execution environment is parsed explicitly; unknown values fail closed.
- Workspace `.env.local` is verified before spawn.
- Child environment is sanitized with `buildChildProcessEnv` before spawn.
- Provider-specific environment additions may supply credentials/config, but `APP_ENV`, `EXECUTION_ENV`, `DATABASE_URL`, `DATABASE_URL_DEV`, and `DATABASE_URL_PROD` are Forge-owned and cannot be overridden by a provider.
- No provider is allowed to inherit a PROD database URL into DEV/LOCAL/TEST execution.
- No changes to V3 lane sequencing, team assignment, or swarm behavior.

## Scoped verification

```bash
pnpm exec tsx --test \
  agent-runtime/gateway/cli-agent-adapter.test.ts \
  agent-runtime/gateway/provider.test.ts \
  agent-runtime/team.test.ts \
  agent-runtime/orchestrate.test.ts \
  agent-runtime/orchestrate-apply.test.ts \
  agent-runtime/repositories.assay.test.ts
```

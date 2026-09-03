# ENG-FORGE-V4-07 — Provider Readiness Gate

## Goal

A Forge runtime can be registered without being executable. Before a logical profile resolves to an adapter, Forge distinguishes:

- registered
- installed
- authentication state
- ready

## Rules

- Profile execution fails closed when the selected adapter is not ready.
- Low-level adapter lookup remains available for diagnostics and unit tests.
- DeepSeek DSH authentication remains delegated to the already-qualified harness; Forge never copies or inspects its credentials.
- Warp requires an executable `WARP_HEADLESS_BIN` plus explicit authentication qualification before it can be ready.
- OpenClaw requires an installed CLI plus explicit authentication qualification before it can be ready.
- `FORGE_WARP_AUTHENTICATED=1` and `FORGE_OPENCLAW_AUTHENTICATED=1` are operator qualification markers, not credentials.
- No new provider is activated by this story.

## Scoped verification

```bash
pnpm exec tsx --test \
  agent-runtime/readiness.test.ts \
  agent-runtime/gateway/cli-agent-adapter.test.ts \
  agent-runtime/gateway/provider.test.ts \
  agent-runtime/team.test.ts \
  agent-runtime/orchestrate.test.ts \
  agent-runtime/orchestrate-apply.test.ts \
  agent-runtime/repositories.assay.test.ts
```

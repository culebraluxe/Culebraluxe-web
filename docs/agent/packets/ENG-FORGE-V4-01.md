# ENG-FORGE-V4-01 — Execution Gateway

## Skills
workflow

## Loop
intent: grow
loop: 1/3

## Test mode
SCOPED

## Goal
Add a provider-neutral Forge execution gateway so existing logical lane profiles can run through DeepSeek, Warp, or OpenClaw without moving workflow authority out of Forge.

## Architect brief
Forge remains the conductor. Story state, lane selection, commit policy, Assay acceptance, and follow rules stay in the existing Forge runtime. The gateway only chooses an execution provider and translates one canonical task into that provider's CLI invocation. Providers return process/run evidence only.

Select the execution provider with `FORGE_EXECUTION_PROVIDER=deepseek|warp|openclaw`. DeepSeek remains the default. Warp uses the current Oz local agent CLI boundary. OpenClaw uses the isolated headless `agent exec` boundary.

Do not let provider-specific nouns leak into AgentWorkCommand or Story Board state. Do not add schema. Do not change V3 lane semantics.

## Acceptance criteria
- No env selection preserves current DeepSeek behavior.
- `FORGE_EXECUTION_PROVIDER=warp` resolves logical Forge profiles to the Warp/Oz gateway adapter.
- `FORGE_EXECUTION_PROVIDER=openclaw` resolves logical Forge profiles to the OpenClaw gateway adapter.
- Unknown providers fail closed.
- Warp/OpenClaw operate in the existing isolated worker cwd when supplied.
- Provider runtime cannot decide story status, follow lane, or Assay acceptance.
- Existing builder/scout/verifier logical profiles remain the workflow-facing contract.

## Assay commands
- node --test agent-runtime/gateway/provider.test.ts

## Out of scope
- Neon schema
- V3 lane changes
- two-story lock
- provider-specific workflow state
- automatic OpenClaw swarm topology
- Warp cloud environments
- control-room UI

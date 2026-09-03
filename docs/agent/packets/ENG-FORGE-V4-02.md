# ENG-FORGE-V4-02 — Gateway Control Surface

## Skills
workflow

## Loop
intent: grow
loop: 2/3

## Test mode
SCOPED

## Goal
Make the new execution gateway visible in the existing TECH Engineering Cockpit without duplicating the cockpit or changing workflow state.

## Architect brief
Surface the active `FORGE_EXECUTION_PROVIDER` as a compact Forge → Gateway → Provider strip above the existing Engineering Cockpit. This is read-only operational visibility. Existing run history remains the source for runtime adapter, commit, environment, and evidence.

## Acceptance criteria
- TECH shows the active provider resolved by the same gateway helper used by Forge runtime.
- DeepSeek, Warp/Oz, and OpenClaw labels are explicit.
- UI states that Forge retains lane, Smith commit, and Assay acceptance authority.
- No workflow writes, schema changes, or duplicate run-history implementation.

## Assay commands
- node --test agent-runtime/gateway/provider.test.ts

## Out of scope
- changing providers from the browser
- secrets/API key management
- Neon schema
- new story state

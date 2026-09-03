# ENG-FORGE-V4-03 — Lane Provider Routing

## Skills
workflow

## Loop
intent: grow
loop: 3/3

## Test mode
SCOPED

## Goal
Allow Scout, Smith, and Assay to use different execution providers within one Forge story while preserving existing lane semantics.

## Architect brief
Register DeepSeek, Warp/Oz, and OpenClaw adapters together. Resolve provider per logical model profile using `FORGE_PROVIDER_<PROFILE>` with `FORGE_EXECUTION_PROVIDER` as the fallback. No provider may alter Story Board state, lane order, Smith commit ownership, or Assay acceptance semantics.

Example:
- `FORGE_PROVIDER_SCOUT_VOLUME=deepseek`
- `FORGE_PROVIDER_BUILDER_FLASH=warp`
- `FORGE_PROVIDER_VERIFIER_MINI=openclaw`

The TECH Gateway Control surface must show the actual three lane routes, not only the global fallback.

## Acceptance criteria
- Scout, Smith, and Assay can route to different providers in one process.
- Profile override wins over global provider.
- Missing profile override falls back to global provider, then DeepSeek.
- Unknown provider values fail closed.
- All three adapters are registered once; logical profile remains the workflow contract.
- TECH shows Scout / Smith / Assay provider routes separately.
- V3 lane behavior is unchanged.

## Assay commands
- node --test agent-runtime/gateway/provider.test.ts

## Out of scope
- automatic swarm sizing
- provider cost optimization
- browser-side provider changes
- secrets management
- Neon schema

# ENG-FORGE-V4-04 — Warp headless guard

## Architect brief

The current standalone Warp Agent CLI (`warp`, verified against the Sep 2, 2026 docs and an installed v0.2026.08.26 build) is an interactive conversation TUI. It does not expose the previously assumed `oz agent run` or another documented one-shot prompt command suitable for Forge's generic headless process adapter.

Forge must not hang, invent a command, or report Warp as executable when no supported headless contract exists. Selecting Warp therefore fails closed unless the operator supplies `WARP_HEADLESS_BIN`, a wrapper implementing this contract:

`<wrapper> --cwd <worker-worktree> --task <canonical-forge-task>`

This preserves the provider-neutral gateway and leaves a clean seam for Warp Automation Platform or a future documented headless CLI.

## Skills
workflow

## Loop
intent: repair
loop: 1/1

## Test mode
SCOPED

## Acceptance

- Warp provider no longer references `oz`
- Warp provider does not attempt to launch the interactive `warp` TUI as if it were headless
- Warp selection without `WARP_HEADLESS_BIN` fails closed with a useful message
- explicit `WARP_HEADLESS_BIN` produces the documented Forge wrapper command
- DeepSeek/OpenClaw routing unchanged

## Assay commands

- `pnpm exec tsx --test agent-runtime/gateway/provider.test.ts`

## Scope

- `agent-runtime/gateway/warp-provider.ts`
- `agent-runtime/gateway/provider.test.ts`
- this packet

No schema, lane-policy, V3 orchestration, or application-domain changes.

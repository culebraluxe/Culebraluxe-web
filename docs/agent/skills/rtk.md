# Skill: rtk

High-performance CLI proxy that filters/summarizes command output **before it reaches the LLM context** — the token/turn compressor for long shell sessions.

## When to use
- **Lead / Smith** (and any role in a long session): keep context bounded instead of dumping raw output.
- Compress reads: `rtk read <file>`, `rtk tree`, `rtk ls`
- Compress runs: `rtk test` (only failures), `rtk git` / `rtk log` / `rtk diff`, `rtk pnpm`, `rtk err <cmd>` (errors/warnings only), `rtk json`, `rtk deps`

## Wiring status (honest)
Installed (global `~/.local/bin/rtk`). **Not yet wired** as an automatic turn-compressor inside the runtime adapter — use it in commands to keep context bounded.

## Never
- Run `git diff`/`pnpm`/`test` and paste full output when the `rtk` variant gives the compact slice.

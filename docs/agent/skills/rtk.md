# Skill: rtk

High-performance CLI proxy that filters/summarizes command output **before it reaches the LLM context** — the token/turn compressor for long shell sessions.

## When to use
- **Lead / Smith** (and any role in a long session): keep context bounded instead of dumping raw output.
- Compress reads: `rtk read <file>`, `rtk tree`, `rtk ls`
- Compress runs: `rtk test` (only failures), `rtk git` / `rtk log` / `rtk diff`, `rtk pnpm`, `rtk err <cmd>` (errors/warnings only), `rtk json`, `rtk deps`

## Wiring status (honest)
Installed (global `~/.local/bin/rtk`, v0.48). **Now auto-instructed:** when `rtk` is
resolvable (PATH scan or `RTK_BIN` override) a context-compression directive is
injected into every Forge model role's run (`buildRtkCompressionDirective` in
`agent-runtime/run-guardrails.ts`, wired in the Forge role-runner) telling the role
to route large output through `rtk`. It never claims a compressor that is absent
(presence-gated). The deeper "automatic transparent compressor inside the runtime
adapter" is NOT wired — the DeepSeek adapter delegates to the external `dsh`
harness, so per-tool compression has no clean in-repo seam there; that stays a
future adapter-level change. Use `rtk` in commands to keep context bounded.

## Never
- Run `git diff`/`pnpm`/`test` and paste full output when the `rtk` variant gives the compact slice.

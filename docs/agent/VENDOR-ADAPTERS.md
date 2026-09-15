# Vendor adapters

The handbook lives in this repo:

- `AGENTS.md` — rules
- `docs/agent/skills/` — reusable task packs
- `docs/agent/packets/` — per-story brief
- `docs/agent/MEMORY.md` — decisions that must survive tool changes

Vendor files (`CLAUDE.md`, Warp rules, Cursor rules, Cline rules) are one-line pointers at that handbook. They are not a second source of truth.

If a tool insists on its own filename, add a pointer. Do not paste the handbook into the vendor file. That is how split-brain starts.

Forge A1 (poller → DeepSeek Smith → Assay) does not read vendor files. Warp / OpenClaw / Claude Code in V3 may. They still commit through Forge unless a human says otherwise.

## What is generated, and what is still hand-written (PIRATE-01, 2026-09-15)

A pointer has two halves, and only one of them should be typed by hand.

The hand-written half is the sentence that says *read the handbook* — `CLAUDE.md` opens with
"Read `@AGENTS.md`", and that judgment belongs to a person.

The generated half is the block between `<!-- FORGE:HANDBOOK:BEGIN -->` and
`<!-- FORGE:HANDBOOK:END -->`: the four load-bearing rules, the four paths to read, and the four
commands. It lives in `lib/agent-vendor-block.ts` and is written by `pnpm forge:sync-agents`, which
replaces only that span, writes nothing when the render is unchanged, appends when the markers are
missing, and **never creates a vendor file that did not already exist** — this doc's rule about not
pasting the handbook into a vendor file is enforced by the tool, not by memory.

Two gates keep it honest, both run by `pnpm forge:packet-lint`:

- a block that differs from a fresh render fails (`vendor-block-drift`);
- a guardrail whose backing sentence is gone from `AGENTS.md` fails (`guardrail-anchor-missing`), so a
  generated file can never assert a rule the handbook stopped carrying. Each guardrail in
  `GUARDRAILS` names its `anchoredBy` sentence for exactly this check.

`pnpm forge:harness` runs both gates plus the scope-manifest freshness check and the harness tests.
Per-machine agent-tool directories at the repo root (`.claude/`, `.cursor/`-family, ~50 of them) are
git-ignored: they are tool state, not this project's source, and none of them is a source of truth.

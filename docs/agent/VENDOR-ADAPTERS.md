# Vendor adapters

The handbook lives in this repo:

- `AGENTS.md` — rules
- `docs/agent/skills/` — reusable task packs
- `docs/agent/packets/` — per-story brief
- `docs/agent/MEMORY.md` — decisions that must survive tool changes

Vendor files (`CLAUDE.md`, Warp rules, Cursor rules, Cline rules) are one-line pointers at that handbook. They are not a second source of truth.

If a tool insists on its own filename, add a pointer. Do not paste the handbook into the vendor file. That is how split-brain starts.

Forge A1 (poller → DeepSeek Smith → Assay) does not read vendor files. Warp / OpenClaw / Claude Code in V3 may. They still commit through Forge unless a human says otherwise.

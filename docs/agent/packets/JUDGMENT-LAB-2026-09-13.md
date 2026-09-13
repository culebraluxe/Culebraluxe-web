# Judgment lab — 2026-09-13 index

Grok does not keep a side worktree. Code already on `main`. This file exists so Deep can find it without archaeology.

## Already on main (do not re-implement)

| SHA | What |
|---|---|
| `d1f802a` | `docs/agent/FORGE-WORKSHOP.md` rewrite. Tools stay. Desk vs cabinet. Serena bounded. |
| `f841af6` | Chat JSON must not beat rows on Lead or Smith. Smith exit = worktree HEAD. |
| `d3f87f4` | Lead PRE reads fields only. Stop teaching `LEAD_ROUTING`. |

Floor: **`d3f87f4`**. A patch that teaches or parses `LEAD_ROUTING` / `LEAD_PLAN` / `SMITH_CANDIDATE` as authority is a regression.

Architect / Scout findings still ingest `FORGE_ARCHITECT_HANDOFF` / `FORGE_FINDINGS_JSON` because those hats have no field writer yet. Do not delete those parsers until a writer exists. Do not teach them to Lead or Smith.

## Packet just landed (not implemented)

`docs/agent/packets/ENG-FORGE-WARM-SESSION-01.md`

One OpenCode session per execution generation. Roles stay. Cold processes go. Do not start this until your tree is clean and the three SHAs above are in the merge.

## Not in git (chat only — operator has the text)

- Deep handoff message (warm session + fields authority)
- Operator changelog past commit subjects
- ReActNet (arXiv 2609.05774) mapping: compile graph per story = FAST/FEATURE/SPLIT/REPLAN; do not emit a new N×N chat topology

## Rule Deep broke

Do work, commit work. Uncommitted hours are the same failure as JSON-as-authority: the model felt done, the cabinet was empty.

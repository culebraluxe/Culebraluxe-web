# Forge lanes

The Neon `storyboard_story` row is the packet. It is not a second board.

| Story column | Lane |
|---|---|
| `context_refs` | Scout packet |
| `architect_brief` | Architect output (often written by a human) |
| `goal` / `scope` / `acceptance_criteria` / `preconditions` / `postconditions` | What Smith implements and Assay checks |
| `agent_work_item.role` + `model_profile` | Lane launch envelope |
| `storyboard_story_run` | Evidence |

Human Architect notes already satisfy Smith. Do not run a model Architect just to copy a brief that is already on the row.

Pipeline: `scout → architect (or human brief) → smith → inspector → assay`

Inspector must be a different lineage from Smith and needs an inline diff. Do not register `reviewer-other` on `deepseek-harness`.

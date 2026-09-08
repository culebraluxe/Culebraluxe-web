# Skill packs

Short markdown the A1 worker or V3 planner loads when the packet lists them.

```md
## Skills
neon, planner
```

Known ids: `neon`, `forms`, `workflow`, `ui`, `planner`, `ripwire`, `serena`, `rtk`, `semgrep`, `knip`, `cruiser`.

`planner` is for the outer loop only. Do not list it on a Smith packet unless
the story is itself about planning docs.

## Tool → role map (analyzer tools)

| id | Role / stage | Purpose |
| --- | --- | --- |
| `ripwire` | Scout · Architect · Lead · Smith · QA | repo intel, blast radius, test surface; estimator multiplier |
| `serena` | Architect · Lead · Smith · Inspector | semantic navigation, references, safe rename |
| `rtk` | Lead · Smith (long sessions) | token/turn compression of command output |
| `semgrep` | Assay · QA | static/security/dataflow |
| `knip` | Maintenance · hygiene | unused files/exports/deps |
| `cruiser` | Assay · QA | architecture boundaries + cycles (hard gate) |

List the analyzer id on a packet only for the role that needs it. Several of
these tools are installed but **not yet wired** into the Forge runtime — the doc
states the honest wiring status; do not claim a tool ran when it did not.


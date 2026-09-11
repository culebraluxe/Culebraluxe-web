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

## Wiring status (the machine-checked source of truth)

Role → tool authority is declared in
`workflow_app/forge/forge-tool-catalog.ts` and enforced by
`resolveForgeToolPermissions(position)`. It is recomputed on **every** lane
transition and holds no session state, so a resumed session cannot carry a stale
grant.

| tool | class | positions | may mutate | wired |
| --- | --- | --- | --- | --- |
| `ripwire` | model-facing | Scout · Architect · Lead PRE · Smith · Inspector | — | yes |
| `serena` | model-facing | Architect · Lead PRE/SOLO/POST · Smith | Lead SOLO/POST · Smith | **no** |
| `rtk` | transparent shim | Architect · Lead · Smith | — | **no** |
| `cruiser` | deterministic | Assay · Inspector | — | **no** |
| `semgrep` | deterministic | Assay · Inspector | — | **no** |
| `knip` | deterministic | Inspector | — | **no** |

Two rules the table enforces mechanically:

- **A deterministic instrument or a transparent shim must never appear in a model
  catalog.** `modelForbiddenTools()` lists them; the tests assert none is offered
  to any position.
- **`wired: false` means no lane may report that the tool ran.** An unavailable
  tool degrades explicitly, with a named fallback, rather than silently
  broadening anything.

`serena` is excluded entirely from Scout, Inspector, Assay and DEV_OPS in the
initial cut — exclusion, not a smaller grant.



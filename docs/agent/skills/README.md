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

| tool | class | positions | may mutate | can run |
| --- | --- | --- | --- | --- |
| `ripwire` | model-facing | Scout · Architect · Lead PRE · Smith · Inspector | — | yes |
| `serena` | model-facing | Architect · Lead PRE/SOLO/POST · Smith | Lead SOLO/POST · Smith | **no** (no seam) |
| `rtk` | transparent shim | Architect · Lead · Smith | — | **no** (no seam) |
| `cruiser` | deterministic | Assay · Inspector | — | **no** (tool not installed) |
| `semgrep` | deterministic | Assay · Inspector | — | yes (informational) |
| `knip` | deterministic | Inspector | — | **no** (tool not installed) |

`can run` is the honest column: it requires **both** the execution seam and the
tool itself. `runStaticGate`
(`workflow_app/forge/forge-static-gate.ts`) is the seam for the deterministic
trio; it runs from the Assay adapter against the exact candidate. Architecture
(`cruiser`) is the **hard gate**; `semgrep` and `knip` are informational and must
never recall Smith.

Interrogate or run them yourself:

```sh
pnpm forge:tools                 # wiring status for every tool
pnpm forge:tools --role smith    # what is in force for one position
pnpm forge:tools --run           # run the deterministic instruments here
```

**Corrected twice on 2026-09-11** — worth recording because both mistakes were
the same mistake:

1. This table said "not yet wired" for all five, and the catalog asserted
   `wired: false` for all five. The status came from this doc instead of the code.
2. Reading the code then showed a real seam for cruiser/semgrep/knip, so all three
   were marked wired — but **neither `dependency-cruiser` nor `knip` is installed
   in this repo**, so the architecture hard gate was silently skipping and
   reporting clean. `wired` now means *runnable*, and the gate reports
   `archRan: false` instead of a false PASS.

Two rules the table enforces mechanically:

- **A deterministic instrument or a transparent shim must never appear in a model
  catalog.** `modelForbiddenTools()` lists them; the tests assert none is offered
  to any position.
- **`wired: false` means no lane may report that the tool ran.** An unavailable
  tool degrades explicitly, with a named fallback, rather than silently
  broadening anything.

`serena` is excluded entirely from Scout, Inspector, Assay and DEV_OPS in the
initial cut — exclusion, not a smaller grant.



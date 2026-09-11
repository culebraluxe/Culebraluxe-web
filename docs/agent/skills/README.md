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
| `serena` | model-facing | Architect · Lead PRE/SOLO/POST · Smith | Lead SOLO/POST · Smith | yes (registered) |
| `rtk` | transparent shim | Architect · Lead · Smith | — | yes |
| `cruiser` | deterministic | Assay · Inspector | — | yes |
| `semgrep` | deterministic | Assay · Inspector | — | yes (informational) |
| `knip` | deterministic | Inspector | — | yes (informational) |

`can run` is the honest column: it requires **both** the seam and the tool.
Three seams exist now:

- **deterministic trio** — `runStaticGate`
  (`workflow_app/forge/forge-static-gate.ts`), run from the Assay adapter against
  the exact candidate. Architecture (`cruiser`) is the **hard gate**; `semgrep` and
  `knip` are informational and must never recall Smith.
- **rtk (V5-24)** — `applyRtkToEnv` (`workflow_app/forge/forge-tool-seams.ts`)
  generates `git`/`ls`/`tree`/`gh` shims for the worktree and prepends them to the
  harness child PATH, so the model keeps typing `git status` and transparently gets
  `rtk git status`. The shim `exec`s the proxy, so exit codes are preserved exactly.
- **serena (V5-23)** — registered with OpenCode 2026-09-11
  (`opencode mcp add serena -- serena start-mcp-server --project <repo>`) and
  verified connected with `opencode mcp list`. The per-position tool list comes
  from `serenaAllowedToolsForRole` in `workflow_app/forge/forge-tool-seams.ts`;
  Architect holds read-only, Smith holds the bounded write subset, and Scout,
  Inspector, Assay and DEV_OPS hold none.

Interrogate or run them yourself:

```sh
pnpm forge:tools                 # status for every tool
pnpm forge:tools --role smith    # what is in force for one position
pnpm forge:tools --run           # run the deterministic instruments here
```

**Corrected three times on 2026-09-11**, and every correction was the same
mistake — claiming a tool worked without executing it:

1. "Not yet wired" for all five, taken from this doc instead of the code.
2. "Wired" for three, because a code path and an import existed.
3. Still not runnable, because **neither `dependency-cruiser` nor `knip` was
   installed** — so the architecture hard gate was silently skipping while reading
   clean. Both were installed on 2026-09-11; the gate then found **6 real
   violations** on its first live run (components importing `db/` directly).

Rule: verify a tool by running it.

Two rules the table enforces mechanically:

- **A deterministic instrument or a transparent shim must never appear in a model
  catalog.** `modelForbiddenTools()` lists them; the tests assert none is offered
  to any position.
- **`wired: false` means no lane may report that the tool ran.** An unavailable
  tool degrades explicitly, with a named fallback, rather than silently
  broadening anything.

`serena` is excluded entirely from Scout, Inspector, Assay and DEV_OPS in the
initial cut — exclusion, not a smaller grant.



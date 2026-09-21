# Legacy TypeScript Server Code

The port moved the domain to Rust. This file is the plan for the TypeScript that used to hold it — so an agent knows
what is retired, what is still serving, and what to do when it touches either.

## The split

| | where | status |
| --- | --- | --- |
| UI — `app/`, `components/` | main tree | **Current.** TypeScript is the right language for the browser. |
| UI transport — `lib/rust-api/` | main tree | **Current.** The thin client that calls the Rust API. |
| Domain/server — `db/`, `services/`, `workflow_app/` | **`legacy/`** | **Retired.** Moved out of the main tree. Do not extend. |

The rule and the anti-patterns live in `AGENTS.md` → "Rust First". `legacy/README.md` says what the moved tree is and
why its test suite is not the application's gate.

## The move happened

The retired stack was moved to `legacy/` — 968 files, one commit, nothing deleted:

```
db/  services/  workflow_app/   ->   legacy/db/  legacy/services/  legacy/workflow_app/
```

Callers keep working: imports crossing the new boundary became aliases (`@/legacy/db/...`, which tsconfig resolves
from anywhere), relative climbs gained the level their file gained, and the committed artifacts that cite source paths
were repointed. `npx tsc --noEmit` is clean across the repository and `pnpm build` succeeds.

`legacy/typescript-server` still points at the last commit where the TypeScript server stack was complete and live,
which is what makes this a move rather than a loss.

## What "retired" means concretely

1. **Do not add a feature to it.** If it needs new behaviour, the behaviour belongs in Rust and the legacy module gets
   either nothing or a call into the Rust API.
2. **Do not "fix it while you are there" beyond the change you came for.**
3. **Convert what you are already editing** — if a task touches `psql_query`/`sql_literal` or a hand-built SQL string,
   convert *that file* to binds as part of the change. Not the directory: the file in hand.
4. **Delete a legacy module only when the Rust path that replaces it is serving.** `docs/rust-parity-ledger.md` is the
   record of which capability serves where. Until a row says Rust serves it, the TypeScript is not dead code.

## Why the rest was not deleted

Because roughly a thousand files import each other and the Next application still builds them. A wholesale removal is
not a cleanup, not a project: it needs the parity rows genuinely green first, and each removal verified against the
route that used it. Its tests fail (19 assertions) and that is expected — they test retired code.


## The branch

`legacy/typescript-server` points at the last commit where the TypeScript server stack was complete and live. It exists
for archaeology: when someone needs to know how a rule used to behave, that branch is a working checkout of it, without
the main branch carrying a frozen copy around.

```
git log legacy/typescript-server -- db/          # how a rule used to behave, before the move
git log legacy/typescript-server -- workflow_app/
```

Nothing is deleted until the ledger says Rust serves that path, and deletions happen one capability at a time with the
route's parity row updated in the same commit.

## When the last row goes green

Then the deletions are mechanical, and the branch is the record. Until then, this file plus the ledger is the answer to
"is this file still alive?" — the ledger for *what serves production*, this file for *what to do about the rest*.

Then the deletions are mechanical, and the branch is the record. Until then, this file plus the ledger is the answer to
"is this file still alive?" — the ledger for *what serves production*, this file for *what to do about the rest*.

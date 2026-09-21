# Legacy TypeScript Server Code

The port moved the domain to Rust. This file is the plan for the TypeScript that used to hold it — so an agent knows
what is retired, what is still serving, and what to do when it touches either.

## The split

| | files | status |
| --- | --- | --- |
| UI — `app/`, `components/` | ~300 | **Current.** TypeScript is the right language for the browser. |
| UI transport — `lib/rust-api/` | small | **Current.** The thin client that calls the Rust API. |
| Domain/server — `db/`, `services/`, `workflow_app/`, server parts of `lib/` | ~1000 | **Legacy in place.** Being retired. Do not extend. |

The rule and the anti-patterns live in `AGENTS.md` → "Rust First". This file is the migration plan.

## What "legacy in place" means concretely

A legacy module stays where it is, keeps working, and stops being a place to add things:

1. **Do not add a feature to it.** If a legacy module needs new behaviour, the behaviour belongs in Rust and the legacy
   module gets either nothing or a call into the Rust API.
2. **Do not "fix it while you are there" beyond the change you came for.** A drive-by refactor of legacy code costs
   review time and buys nothing, because it is going away.
3. **Convert what you are already editing.** If a task requires touching `psql_query`/`sql_literal`, or a hand-built SQL
   string, convert *that file* to binds as part of the change. Not the whole directory — the file in hand.
4. **Delete a legacy module only when the Rust path that replaces it is serving.** `docs/rust-parity-ledger.md` is the
   generated record of which capability serves production where. Until a row says Rust serves it, the TypeScript is not
   dead code, it is the live path.

## The branch

`legacy/typescript-server` points at the last commit where the TypeScript server stack was complete and live. It exists
for archaeology: when someone needs to know how a rule used to behave, that branch is a working checkout of it, without
the main branch carrying a frozen copy around.

```
git branch legacy/typescript-server      # created at the commit before any deletion begins
git log legacy/typescript-server -- db/  # how a rule used to behave
```

Nothing is deleted from the main branch until the ledger says Rust serves that path, and deletions happen one capability
at a time with the route's parity row updated in the same commit.

## Why not delete it all now

Because roughly a thousand files import each other, and the Next application still builds them. A wholesale removal is
not a cleanup, it is a project: it needs the parity rows to be genuinely green first, and each removal verified against
the route that used it. Doing it early would take the app down for capabilities that have not been cut over, which is
exactly the failure this whole port has been careful to avoid.

## When the last row goes green

Then the deletions are mechanical, and the branch is the record. Until then, this file plus the ledger is the answer to
"is this file still alive?" — the ledger for *what serves production*, this file for *what to do about the rest*.

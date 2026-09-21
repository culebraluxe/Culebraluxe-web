# NAMING AND PROTECTION — how things are named, and what must not be broken

The captain, 2026-09-18: *"we should have a naming convention guide and a do not touch file list or
something … it's so obvious to me, don't break something that is already there."* This is both halves.
The second half is **enforced** — `scripts/protected-files.test.ts` runs inside `pnpm test:harness`,
which the release build runs before anything ships, so a broken protection stops the line instead of
being noticed weeks later.

## Part 1 — naming

| thing | shape | example |
|---|---|---|
| migration | `NNN_snake_case_purpose.sql`, zero-padded, next free number | `188_sprint_accounting.sql` |
| story | `AREA-LANE-TOPIC-NN` — the batch/sprint is separate | `ENG-FORGE-QA-VERDICT-VOCAB-01` |
| sprint | `S<number>`, where `number` **is** the batch | `S98` |
| fence (a story's proof) | `<subject>.test.ts` in `legacy/workflow_app/tests/` | `claim-clock.test.ts` |
| generated manifest | `docs/agent/manifest/<SCOPE>.md` | `docs/agent/manifest/all.md` |
| handbook / guide | `docs/agent/UPPER-CASE-TOPIC.md` | `docs/agent/TEST-BUDGET.md` |
| operator script | `scripts/<verb-or-noun>.ts`, header comment answering one question | `scripts/sprint.ts` |
| view / table | `storyboard_*` (the board), `forge_*` (the engine), `l_*` (live intake) | `storyboard_sprint` |
| package script | `area:thing` — `forge:`, `sprint:`, `db:`, `test:` | `test:story` |

Three habits that matter more than the shapes: a new *number* is always the next one (never reuse a
taken migration or sprint number), a new *file* gets a header comment naming the question it answers,
and a *fence* is named after the behaviour it proves rather than the ticket that asked for it.

## Part 2 — protected: do not break what is already there

The failure this list guards against is mechanical, not malicious: a tool writing a file it does not
understand, a generator whose output path collides with a hand-written table, a rename taking the wrong
sibling. On 2026-09-18 `pnpm forge:manifest COLUMN-WRITER-AUDIT` overwrote a 128-column audit table
with a 34-row index skeleton — only git brought it back.

| file | what must stay | why |
|---|---|---|
| `AGENTS.md` | `# CulebraLuxe Agent Operating Context` | the always-read handbook; a lane that rewrites it rewrites its own rules |
| `docs/agent/ORIENTATION.md` | `# ORIENTATION — the map (start here)` | the entry point to everything else |
| `docs/agent/MEMORY.md` | `# Decision log` | appended, never rewritten — a decision edited away is one we make again |
| `docs/agent/CURRENT.md` | `# Current Machine — Forge SDLC` | the current-state claim read first |
| `docs/agent/releases.md` | `# Releases — what was actually built, deployed and probed` | the release receipts: the only durable evidence of what PROD served |
| `docs/agent/COLUMN-WRITER-AUDIT.md` | `# Column writer audit` | the classified column table |

**A directory rule, not just files:** every `*.md` in `docs/agent/manifest/` must start with
`# Scope manifest`. The freshness gate renders a fresh manifest per name and refuses one that differs —
so anything *else* written there can never satisfy it. An audit, a report or a plan belongs outside
that directory.

### Rules that are not a list

0. **A new top-level page must be indexed.** Every page in `docs/agent/` is a row in
   `docs/agent/manifest/all.md`; adding one without `pnpm forge:manifest all` leaves the harness
   DRIFTED and blocks the release build (it caught exactly that on 2026-09-18, twice: batch 91's
   `route-authority-manifest.md` and this file).
1. **Never rewrite an applied migration.** The `schema_migration` ledger records each file by checksum
   and refuses a changed one. Append a new migration instead; if a correction is needed before prod
   sees it, re-apply with `--force` and a note (that is what 187 did).
2. **Never edit a fence to make it pass.** The fence is the story's proof; adjusting it to fit the code
   is the one move that makes every other gate meaningless. Change the code, or change the story.
3. **Generated files are regenerated, never hand-edited.** `docs/agent/manifest/*.md` says so in its
   own header.
4. **A tool must not overwrite what it cannot recognise.** If you write a script that takes a path or a
   name, make it check what is already there — `nonManifestRefusal` in `scripts/forge-manifest.ts` is
   the pattern.

## Adding to the list

Add the path, its marker and a reason to `PROTECTED_FILES` (or a directory rule to
`PROTECTED_DIRECTORIES`) in `scripts/protected-files.ts`. The test requires a marker **and** a reason of
real length, so the list stays a conversation rather than a set of names: a protection nobody can weigh
later is a protection nobody will keep.

# PIRATE-01 — pirate seven ideas from OpenContext, in our shapes

## Goal

Turn the seven accepted items of the OpenContext steal list into working, gated parts of this repo: a
ranked scope manifest, regenerable vendor pointer blocks, a skill-anchor check, guardrails replicated
from one constant, evidence/citation discipline, machine-readable gate output, and a post-deploy live
smoke. Reimplemented against our files, our lint, and our commands — no vendor code, no new dependency,
no store outside this repo.

## Why

The deep dive on 2026-09-15 (`0xranx/OpenContext`) produced a ranked steal list; the captain cleared
items 1-7 and parked 8-9 (RRF fusion, description triage) as premature, with 10-13 rejected. Their
manifest is a directory listing ordered by `rel_path` and their guardrail block is written into users'
home directories with no test file behind it. What is worth taking is the **mechanism**: a generated,
regenerable artifact that a gate can check, instead of prose that only a human maintains.

Measured before building (see "Verified premises"): plain `grep -ril` over `docs/agent` matches 124 of
133 files, alphabetically. Ranking is the whole difference between "read these" and "here are 124
files".

## Scope

`docs/agent/manifest/<SCOPE>.md` (generated), `scripts/forge-manifest.ts`, `scripts/forge-sync-agents.ts`,
`scripts/prod-smoke.ts`, `lib/scope-manifest.ts`, `lib/agent-vendor-block.ts`, `scripts/forge-packet-lint.ts`
(rules added only), `scripts/forge-batch-status.ts` (`--format json`), `AGENTS.md` (evidence section),
`CLAUDE.md` (now generated block), `docs/agent/VENDOR-ADAPTERS.md` (what is generated vs hand-written),
`package.json` (four scripts), `.gitignore` (agent-tool dirs), and tests.

Out of scope: embeddings, vector stores, Rust/Docker, a store outside the repo, engine lane changes,
Neon schema changes, the release train and DEV_OPS receipt.

## Architect brief

Two shapes, both already used in this repo.

1. **Pure logic in `lib/`, IO in `scripts/`** — so the ranking and rendering can be asserted without a
   filesystem, and the CLI stays a thin shell. Same split as `lib/story-moves.ts` + its probe.
2. **One gate command** — new checks become rules inside `pnpm forge:packet-lint` (which already owns
   "scan the harness, not the app") rather than a fifth command the captain has to remember. New tests
   are wired into a runner that exists, because `scripts/forge-packet-lint.test.ts` was written on
   2026-09-15 and **nothing ran it** until now.

## Context refs

- `scripts/forge-packet-lint.ts` — rules 1-7, `citedRepoPaths`, `pathExists`, `loadHarnessFiles`.
- `lib/story-moves.ts` — the pure-logic/IO split this follows.
- `scripts/forge-batch-status.ts` — the read-only state command that gains `--format json`.
- `scripts/vercel-deploy-prod.sh` — the deploy that gains a live smoke.
- `agent-runtime/repo-context.ts` — where retrieved material is assembled into a prompt.
- `docs/agent/VENDOR-ADAPTERS.md` — the doctrine the generated block must not contradict.

## Acceptance criteria

1. `pnpm forge:manifest PIRATE-01` writes `docs/agent/manifest/PIRATE-01.md`; identity lanes (packet,
   cited paths, story commits) come first, lexical entries after; every row carries why/last-touched;
   a cited path that does not exist is marked and fails the lint.
2. `pnpm forge:manifest PIRATE-01 --check` exits 1 when the file on disk has drifted from a fresh
   render, 0 when it matches.
3. `pnpm forge:sync-agents` rewrites only the marker block in vendor pointer files, writes nothing when
   the content is unchanged, and `--check` fails on a hand-edited block.
4. `pnpm forge:packet-lint` reports the new rules; a manifest citing a deleted path, a vendor block that
   drifted, and a citation past the end of a file all fail.
5. `pnpm test:harness` runs the script-level tests (including the previously orphaned packet-lint test).
6. `pnpm smoke:prod` checks the live site and fails loudly on a missing marker or an unexpected sha.

## Preconditions

Green tree at `57798d2`. No Neon change, so no DEV/PROD schema work.

## Postconditions

`pnpm forge:packet-lint`, `pnpm test:harness`, `pnpm forge:manifest --check`, and `pnpm smoke:prod` all
run and are quoted in the report. Vendor files remain pointers; no second source of truth is created.

## Skills

workflow

## Loop

Empty on the first pass.

## Test mode

SCOPED

## Assay commands

- node --import tsx --test scripts/forge-packet-lint.test.ts scripts/forge-manifest.test.ts scripts/forge-sync-agents.test.ts
- node --import tsx scripts/forge-packet-lint.ts
- node --import tsx scripts/forge-manifest.ts PIRATE-01 --check
- git diff --check

## Verified premises (checked against HEAD, 2026-09-15)

| Claim | Reality |
|---|---|
| `grep` is good enough for the docs corpus | **FALSE** — 124 of 133 files match a five-word query, alphabetically |
| RRF/tf-idf fusion beats grep | **FALSE for exact identifiers** — `V5-23` grep = 1 file, correct; a fused rank diluted it. Structural signals beat statistical ones here |
| `scripts/forge-packet-lint.test.ts` is run by CI or a script | **FALSE** — no package.json script references it |
| `CLAUDE.md` is hand-written and unchecked | **TRUE** — it is a pointer with no gate |
| `vercel-deploy-prod.sh` verification is clean | **FALSE** — the sha comparison and both print lines are duplicated (lines 88-91 and 93-97) |

## Out of scope, recorded so it is not silently lost

Item 8 (RRF fusion) measured worse than plain tf-idf on this corpus: every top-5 score landed inside a
0.0016 band and the ordering collapsed to "most recently committed". Item 9 needs a fresh triage field
first. Both parked by the captain on 2026-09-15.

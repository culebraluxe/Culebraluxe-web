# Live TypeScript — the port queue, and who to ask before touching it

The gate is `pnpm ts:count` (`scripts/live-ts-gate.sh`, baseline `docs/agent/live-ts-baseline.txt`, CI step
`live-TS count (may only fall)` in the `static gates` job). **69 files at 2026-09-29. The baseline may only fall.**

This file is the other half of `docs/agent/BROKEN-TS-INVENTORY.md`, and the split is the whole point: that
inventory lists files that **cannot load**, this one lists files that **load and run**. `broken:ts:sweep` was
green for a week while 69 working TypeScript files sat outside `legacy/`, which is how a green sweep came to be
read as "the TypeScript is retired". Re-derive the list any time with `pnpm ts:count:list`.

## Ask the engine owner before deleting or moving anything here (33 files)

`agent-runtime/` (29) is the worker's body — lanes, roles, the harness client, gateway providers, the assay
arithmetic — and these four are the entry points around it:

`scripts/agent-scheduler.mjs` · `scripts/agent-scheduler.test.mjs` · `scripts/forge-engine-worker.ts` ·
`scripts/forge-silent-failure-gate.ts`

They are named by `package.json` commands and launchd installs that the operator runs. **Nothing here is deleted
on a Task-3 sweep without the owner of those files agreeing first** — the captain asked for exactly that check on
2026-09-29, because a lane deleting the worker that drives it is a self-inflicted outage.

## Ask first, and update the references in the same change (6 files)

`workflow_engine/` (4: `lib/workflow/{engine,errors,expressions,types}.ts`) and `testv2/engine_tests/{fake-sql,fixtures}.ts`.

Nothing imports them, and `legacy/` is their natural home — the repository's retirement rule is that the old stack
lives in `legacy/`, and `legacy/workflow_app/README.md` already links to `../workflow_engine/lib/workflow/types.ts`,
a link that is broken today and would become correct. But they are not inert: `agent-runtime/test-mode.ts` forbids
their test globs, `agent-runtime/learn-loop.ts` skips `testv2/`, and `.dependency-cruiser{,.runtime}.js` plus
`.gitleaks.toml` name both paths. Moving them means editing those four references in the same commit, in someone
else's crate. Ask, then move.

## The rest (30 files) — the port order

**Named by a `package.json` command (8).** These are the operator's menu, so porting them changes what the
operator types: `apple-sync-agent.mjs` · `calendar-sync-agent.mjs` · `broken-ts-sweep.mjs` · `dead-command-sweep.mjs` ·
`portal-nav-smoke.mjs` · `protected-files.ts` · `test-section.ts` · `ui-capture-fixtures.mjs`.

**Named by a workflow (1).** `app-runtime-boundary.mjs` (the dependency-cruiser architecture run, via
`.dependency-cruiser.runtime.js`).

**Harness suites the runner discovers by convention (4).** `scripts/*.test.ts` is globbed by
`scripts/test-harness.mjs`, so these are named by their directory, not by a path: `protected-files.test.ts` ·
`rust-parity-ledger.test.ts` · `test-sections.test.ts` · `workflow-cli.test.ts`.

**Reached by a live sibling (10).** `rust-live-check/{_env,apple-mail,apple-messages-intake,engine-routes,pool-counters}.mjs`
(the DEV live-check kit `docs/rust-contributing.md` points at) · `apple-local-listener.mjs` · `rust-parity-ledger.ts` ·
`test-sections.ts` · `harness-authorization.ts` · `route-authority-manifest.ts`.

**Named only in this baseline and in documents (7) — the cheapest wins, measured not assumed.** Each stem was
searched across the tree (`git grep -l --fixed-strings <stem>`) and none has a code caller left:
`check-trailing-whitespace.ts` and `static-page-body.mjs` are named by nothing at all besides this gate's baseline;
`generate-break-glass-hash.mjs` and `verify-break-glass-secret.mjs` survive only in `docs/auth-bootstrap-order.md`,
`docs/auth-test-matrix.md` and the storyboard docs; `merge-contacts-notes.ts` in a handoff; `ui-flip-readiness.mjs` in
`docs/RUST-UI-PORT.md`; `oc-probe3.ts` in a packet and `eslint-suppressions.json`. Two of them (break-glass) touch a
security control, so they are ported deliberately or not at all — never deleted on a sweep.

## What the count means when it hits 0

`ts:count` at 0 is the work order's sentence: any single new `.ts/.tsx/.mts/.cts/.js/.mjs/.cjs` outside `legacy/`
fails the build. The four excluded names (`eslint.config.mjs`, `postcss.config.mjs`, `.dependency-cruiser.js`,
`.dependency-cruiser.runtime.js`) are settings for the tools that check what is left, and they are excluded in one
named line in `scripts/live-ts-gate.sh` rather than by a pattern a new file could hide behind.

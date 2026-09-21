# Review brief — the seam night (2026-09-15/16)

HEAD: `fe7a89a` on `main`. Everything below is pushed, tsc-clean, lint 0 failures. A = held by the captain.

## What a reviewer should open, in the order that matters

1. **QA measures the candidate or not at all** — `legacy/workflow_app/forge/agent-runtime-role-runner.ts` (~1012-1036):
   the assay pins the workspace to `candidateSha` before measuring and throws
   `ASSAY_WORKSPACE_NOT_CANDIDATE` (a gap for a human) if it cannot; worktrees only, never the operator's
   primary checkout. Why: candidate `a54d8639` contained its own proof (185 lines) while the lane reported
   `Could not find …` with `verified=none` and dispatched repair at code it had never checked out. `60edf16`,
   `b33e938`. **Attack:** is there any path where the proofs run in a tree that is not the candidate?
2. **One verdict, one author** — the collapse itself is story `ENG-QA-SINGLE-VERDICT-01` (packet + row, queued).
   Landed tonight are the two worst behaviours: `forge-role-mapping.ts` is a projector (no verdict of its own,
   `verificationGap` survives, a gap is never `CODE_DEFECT` — `61a7b89`) and a refusal quotes the failing
   command's own output (`b320faa`). **Attack:** can a second verdict still be born anywhere?
3. **The seam law is a function, not prose** — `seamForNewFile` in `legacy/workflow_app/forge/forge-shaping.ts`, cited
   by `agents/architect/assess.ts` (its reason now names the *fix*) and by the Architect preamble
   (`legacy/workflow_app/forge/forge-architect-directive.ts`). `8f…`/`d16e330`, `fe7a89a`. **Attack:** does the
   preamble restate the law anywhere else that could drift?
4. **Held is not crashed** — `forge-executor.ts` returns `blockedReason` (`no ready task; active: <node>=<status>`)
   and `scripts/forge-engine-worker.ts` prints it before exit 2. `aa8d548`. **Attack:** any other silent stop.
5. **Receipts are not rows** — `scripts/release-record.sh`: `eligible=yes` requires build+deploy+**live probe**
   agreeing **for that SHA**; the SHA is re-read after the build (a move closes the row); a torn final line is
   never read as a row; `--verify <sha>` refuses without a sha; `--production` answers *what is serving now*
   and claims nothing about any sha; `RELEASE_RECORD_FILE` makes the whole path testable. `574505f`, `51f110c`.
   **Attack:** can an eligible row ever be cited for a SHA it did not measure?
6. **Decisions reach the lanes that decide** — `laneNeedsDecisions` includes `architect` + `inspector`
   (`cdc0ee4`); the Architect preamble says an active decision is *input to the contract* (`fe7a89a`).
7. **A learn packet is committed by the pass that writes it** — `agent-runtime/learn-loop.ts` (~317): current
   branch, no push, failure warns in the worker log and never fails the pass; the untracked backlog is
   committed (`da61f9e`, `cede281`). **Attack:** does that commit race the worker's own `git pull --ff-only`?
8. **The scheduler is a co-writer** — `AGENTS.md` Ask-first: stop it, confirm `pnpm forge:doctor` is clear,
   work, resume. `ca275a2`.

## Known-open, stated rather than hidden

- **B** — `forPolicyLabel` (`cheap | dear`) exists in `lib/forge-kind.ts`; the three human-facing print sites
  (cockpit chip, worker log, ROI) are **not** wired yet. Stored policy values untouched; no routing change.
- **A** — held by the captain: two real stories with kind+policy recorded, scored against Grok's list.
- **`completion: 100` on an unfinished story** (`ENG-QA-SINGLE-VERDICT-01`, status `In Progress`) — first
  entry for the producer-less audit Grok ordered (report first, ratchet on keys that already have a producer).
- **Operator, not a story** — the launchd worker's TCC wall (`Operation not permitted` on `~/Documents`) was
  fixed tonight by granting Full Disk Access to `/bin/bash`; the wrapper's message still says
  `checkout-not-main branch=''` for a *permission* failure, which names the wrong fault.
- **Untracked leftovers for triage** (not mine to delete blindly): `culebraluxe-apple-proof.sh`, `data/`,
  `scripts/_close_stale_runs.mts`, `scripts/_triage_live.mts`, `skills/`.

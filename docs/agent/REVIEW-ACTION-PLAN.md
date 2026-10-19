# External review action plan (Grok, 2026-10-19)

## First correction: the review was stale

Grok reviewed the **public ForgeVCS tree at `15f0442`**; local `main` was `bd43d18` (repo ACL/path
scopes `ffbe5dd`, backups `b937fa0`, rate/TD `bd43d18`). Pushed `main` to `origin`; public and local
now both `bd43d18`. `docs/PRODUCTION-PACKET.md` is now visible. Re-review should be against
`bd43d18`/later, not `15f0442`.

Lane-deep/Forge is private (`github.com/culebraluxe/Culebraluxe-web`). Options: grant reviewer
access, or use a review tarball. No source is exposed publicly.

## Reconcile findings vs. current tree

| Grok finding | Current state | Action |
|---|---|---|
| Identity/isolation M0: single agents group, no per-repo ACL | FVCS-01 landed (`ffbe5dd`): repo ACL, owner claim, default deny | **WO-R3**: narrow `acl.toml` to lane prefixes for a pilot + live ACL proof; update readiness docs |
| No delete/archive API | DevOps-only deletion + GC landed (M1) | Document in packet/readiness (stale Grok) |
| No merge/rebase/resolve endpoints | merge/rebase/merge-land landed; conflict context exists | **WO-R2**: paused-conflict resolve/continue workflow |
| Workspace writes last-writer-wins | No distributed lock | **WO-R4**: workspace write lock/lease (design + tests) |
| Backup retention/offsite/alert gaps | FVCS-05 landed: Mac zip, drill, alerts, retention; Mac zip is accepted offsite | Update docs; no further work unless incident |
| No rate limits/quotas | Deferred to TD-4 by principal decision; time budget N=120 replaces it | No action |
| No load/soak/chaos | Synthetic soak replaced by real-work merge pilot | **WO-R5**: real-work merge pilot |
| Auth-decision retention, no migration runner | Open | **WO-R6**: retention + ordered migration runner |
| Facade ~140 kB growing | Real concern | **WO-R1**: extract mappers/resolver, typed errors, single admin-permission source |
| Policy/admin drift (`admin.repos` vs `admin.refs`) | Docs/code disagree | part of WO-R1 |
| Stringly statuses, large error type | Hygiene | part of WO-R1 |

## Work orders

| WO | Scope | Owner | Status |
|---|---|---|---|
| R1 | ForgeVCS facade decomposition + typed error enum + single admin-permission source; clippy large-err review | ForgeVCS engineer | DONE (facade/ dir, ApiError, AdminCapability; 56 service tests + full gate) |
| R2 | Conflict `resolve`/`continue` endpoints + paused state | Smith | DONE (paused conflicts API+CLI, workspace 137 tests, QA live pending) |
| R3 | `acl.toml` lane-prefix rollout + live ACL proof in pilot | Operator/DevOps | DONE (18-check live PASS: path scopes + repo ACL layers) |
| R4 | Workspace write lock/lease design + tests | Architect | DONE (DB lease + fencing/audit, real-Postgres test) |
| R5 | Real-work merge pilot (mirror real repo, 3–5 agent branches, compare to jj) | Lead | NEXT |
| R6 | Auth-decision retention + ordered migration runner | DevOps | DONE retention pruner (dry-run default); migration runner dropped (single-instance) |

Sequence: R1/R2 in parallel; R3 before R5; R5 then R4 if locking is needed; R6 before production claim.

## External work order — Claude `forgevcs/WORK-ORDER-MULTIAGENT.md` (2026-10-19)

Status: **QUEUED behind the P1 queue; evaluate after P1 streams close, do not start now.**
File: `/Users/Shared/dev/src/forgevcs/WORK-ORDER-MULTIAGENT.md` (10 items, ~180–220h claimed).

Preliminary triage (verify during evaluation):
- **P0.2 idempotency for `claim()`** — ForgeVCS has **no claim operation**; claims belong to Forge
  (`agent_work_item`). This item likely maps to Forge, not ForgeVCS, or is based on a wrong
  assumption. Do not implement until the API surface is confirmed.
- **P0.1 structured error codes** — partially satisfied by the just-landed `ApiError` (typed
  enum + stable codes). Evaluate whether agents need finer retryable-vs-policy classification.
- **P0.4 JSON logging** — likely genuine; assess against existing logs.
- **P1.3 audit trail** — `forgevcs_auth_decisions` exists; gap is retention/query API, already WO-R6.
- **P2.3 sequence ordering** — activation sequence exists; check whether it covers the claimed use.
- **P1.1 backup idempotency** — backups/drills landed (FVCS-05); checkpoint recovery may still be real.
- **P1.2 timeouts** — partially addressed in Forge probe sandbox (P1-C), not necessarily ForgeVCS ops.

Evaluation criteria: keep only items that (a) target an API that exists, (b) aren't already closed,
(c) have a testable acceptance criterion, (d) don't duplicate R2–R6 or the P1 queue. Re-estimate
after dedupe; the 180–220h figure is not accepted yet.

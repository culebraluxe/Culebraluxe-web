# Agent Run Log

## 2026-08-22 — CRM-16 Documents / Signatures / Transaction Packet (parent story close-out)

- Role: Builder
- Story: CRM-16 — Documents / Signatures / Transaction Packet. Story Board record (DEV): Planned/0, note "Parent story. DOC-01 through DOC-05 carry rollup weight." All five children were already implemented and committed (DOC-01 `33ad8b8`, DOC-02 `b0fdaec`, DOC-03 `c4d3939`, DOC-04 `e8616e5`, DOC-05 `a49fca9`; CRM-21 `e9ee5d5` consumes the packet + signed lineage). This run verified the delivered implementation end-to-end and recorded the outcome on the authoritative Story Board (notes-only; status/completion stay the human board decision).
- Files changed: `db/migrations/055_storyboard_crm16.sql` (new — DEV Story Board notes reconciliation for CRM-16 and DOC-01..DOC-05, applied to the disposable DEV branch; status/completion untouched), `docs/agent/RUNLOG.md` (this entry).
- Decision: the parent story carries no code of its own (rollup = false) — the close-out is verification plus an accurate board record, following the exact pattern of the OPS-06/05/PLAT-01 storyboard migrations (051/053/054). The six-story record captures what each child actually delivered: DOC-01 deal-scoped `transaction_document` with checked type/state/source categories, SME `document_type_label`, media-backed signed-artifact lineage and claim-first receipt idempotency (migration 027); DOC-02 the pure derived `transaction-packet` projection with no packet table, jurisdiction-neutral gating, unresolved facts surfaced never fabricated (workflow_app/transaction-packet.ts); DOC-03 the provider-neutral SignatureProvider seam — `signature_request`, neutral status model, interface/registry/fake/status-mapping/application router, signature commands in the canonical registry (migration 036); DOC-04 BoldSign entirely behind that seam — `bold_sign_request` (one row per request + unique envelope id) and `bold_sign_webhook_event` (replay dedupe + durable DOC-05 enqueue) with raw provider state never crossing to canonical tables (migration 037); DOC-05 the completed-event reconciler that downloads the signed artifact ONCE outside the transaction, appends a NEW media row and moves the document sent -> signed via the DOC-01 transition, keyed by a claim-first receipt on the neutral event id (replay = no-op). `workflow_engine` stayed generic and untouched throughout.
- Verification (SCOPED policy): the targeted seam suite ran green in a clean HEAD worktree — `transaction-document.test.ts` + `transaction-packet.test.ts` + `closing-document-readiness.test.ts` + `signature-seam.test.ts` + `boldsign-adapter.test.ts` + `signature-reconciliation.test.ts` 92/92, plus the adjacent `command-inventory.test.ts` (signature command registration) 10/10 — 102/102 total. `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. Live DEV verification: migration 055 applied cleanly via `scripts/apply-migration.mjs` — the six storyboard rows now carry the delivered-outcome notes with status/completion untouched (read-back verified). Full regression not run per runtime policy.
- Scope boundary: close-out of the parent story — no implementation changes were needed (DOC-01..05 shipped in their own commits). Known unrelated working-tree state: the current tree carries in-flight AUTH-05 receipt-actor edits that leave the four receipt-touching seam test fakes mid-update (their fake SELECT matcher is not yet aligned to the new `actor_app_user_id` column), so those four suites fail only on that fake matcher in the dirty tree — that repair belongs to AUTH-05, not this story; the SB-01 storyboard revert is likewise untouched. Story stays Planned on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 055 applied to the DEV branch only; notes-only — no schema change, no business data). Only the two CRM-16 files committed; unrelated in-flight working-tree changes left untouched.

## 2026-08-22 — PLAT-01 Property Source Consolidation (remaining cleanup)

- Role: Builder
- Story: PLAT-01 — Property Source Consolidation. Story Board record (DEV): Partial/50, note "Featured homepage is Neon-backed. Remaining Sanity Portfolio/source consolidation and cleanup remain." The featured homepage was already Neon-backed (commit f4aaaf7) and Sanity property delivery was already retired (commit f0aa00f); this run completed the remaining consolidation/cleanup — every leftover Sanity artifact is now gone from the application surface.
- Files changed: `lib/environment-readiness.ts` (removed `sanityProjectConfigured` — the "declared but not yet wired into code" probe field and its `NEXT_PUBLIC_SANITY_*` reads), `components/portal/system-health.tsx` (removed the "Sanity project configured" readiness row), `workflow_app/tests/environment-readiness.test.ts` (dropped the SANITY env keys from isolation; added a PLAT-01 test asserting the readiness surface no longer exposes Sanity), `package.json` + `pnpm-lock.yaml` (removed the five unused Sanity/PortableText dependencies: `sanity`, `next-sanity`, `@sanity/vision`, `@sanity/image-url`, `@portabletext/react` — verified zero imports in app/components/lib/db/scripts/workflow_engine/workflow_app; lockfile pruned with `pnpm install --lockfile-only`, remaining direct deps untouched), `db/migrations/054_retire_sanity_document_id.sql` (new — drops `property.sanity_document_id` and its `property_sanity_document_unique` constraint; applied to the disposable DEV branch only), `docs/environment-audit.md` (Sanity env vars reclassified retired/legacy, safe to remove), `docs/property-ui-contract.md` + `docs/portal-ui-contract.md` (canonical property/media source is now Neon/Postgres + `media`/`property_media`; Sanity-as-editorial-system statements removed), `docs/agent/RUNLOG.md` (this entry).
- Decision: Sanity is no longer a property/media source — Neon `property` is canonical, `media` + `property_media` own roles/order, and `property.id` is the stable identity (AGENTS.md). The cleanup therefore removes every dead Sanity artifact rather than adding new abstractions: the readiness probe should not report a source the application no longer depends on, the app should not ship an unused CMS toolchain (the `sanity` studio CLI alone pulled in hundreds of transitive packages), and the schema should not carry a column no code reads and no row populates (DEV: 6 properties, zero non-null `sanity_document_id`). Migration 001 is immutable history and was not edited; 054 is the additive cleanup migration. Sanity identity was never relationship identity — nothing references `sanity_document_id` in code, so dropping it changes no behavior.
- Verification (SCOPED policy): targeted `workflow_app/tests/environment-readiness.test.ts` 8/8 pass (7 existing + 1 new PLAT-01 retirement assertion). Adjacent seam green: `navigation-registry.test.ts` 5/5 (system-health route seam). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. `pnpm exec next build --webpack` passed (exit 0) — touched code includes a portal client component + server lib. Live DEV verification: migration 054 applied cleanly (`sanity_document_id` column and `property_sanity_document_unique` gone; all 6 property rows intact; no other schema change). Full regression not run per runtime policy.
- Scope boundary: consolidation = single canonical Neon property/media source + removal of the retired Sanity artifacts (probe field/UI row, dependencies, dead schema column, stale contracts). `.env.local` Sanity vars were deliberately left in place (local, uncommitted config; removal is a human/ops choice documented in `docs/environment-audit.md`). Story stays Partial on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 054 applied to the DEV branch only; no production schema change). Only the PLAT-01 files committed; unrelated in-flight working-tree changes (SB-01 storyboard staging + AUTH-05 actor recording) left untouched.

## 2026-08-22 — OPS-05 Deal / Participant Maintenance (write-side)

- Role: Builder
- Story: OPS-05 — Deal / Participant Maintenance. Story Board record (DEV): Planned/10, note "Write-side maintenance follows participant-model decision." The read side (Deals Portfolio, Deal Workspace, long-tail participant writes) already existed; this run delivered the missing write-side maintenance — deal creation and the structural participant (client/owner/seller) lifecycle — on top of the participant-model decision (migration 034: deal_participant canonical, at most ONE active structural participant per role per deal).
- Files changed: `lib/deal-admin.ts` (new — pure contract: deal-create normalization + structural participant vocabulary; subject kinds fixed per role so the per-deal legacy FK mirrors can stay consistent: client/seller are people, owner is an app user), `db/deal-admin-writes.ts` (new — injected-TxRunner seam: createDeal validates the property/client/owner are present and active, then inserts the deal AND its client/owner deal_participant rows in ONE transaction so the canonical model is correct from birth; setStructuralParticipant ends the active same-role row, inserts the new one, and syncs the per-deal mirrors deal.client_person_id / deal.owner_user_id, mapping a 23505 race to a clean conflict; endStructuralParticipant ends an owner/seller — the client is replaced, never ended, because deal.client_person_id is NOT NULL — clearing the guarded owner mirror. property.seller_person_id is deliberately never synced: property-domain fact, shared across deals, outside deal-participant maintenance), `app/portal/actions.ts` (createDealAction / setStructuralParticipantAction / endStructuralParticipantAction through portalWrite('deal.write')), `db/deals.ts` (listDealableProperties — active-property picker), `app/portal/deals/page.tsx` + `app/portal/deals/[dealId]/page.tsx` (owner-candidate + property props), `components/portal/write/deal-create-panel.tsx` (new — New deal panel on the Deals Portfolio; lands on the new workspace), `components/portal/write/structural-participant-actions.tsx` (new — set/replace client/owner/seller, end owner/seller in the Deal Workspace), `components/portal/deals-portfolio.tsx` / `deal-workspace.tsx` (wiring), `docs/auth-command-map.md` + `workflow_app/tests/persistence/auth-03.test.ts` (three new deal.write actions; write inventory 31 -> 34), `workflow_app/tests/deal-admin.test.ts` (new — 29 targeted unit tests, zero Neon), `db/migrations/053_storyboard_ops05.sql` (new — DEV Story Board notes reconciliation; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: the participant-model decision says deal_participant is canonical and the legacy FKs remain as mirrors read projections no longer use. Because deal.client_person_id is NOT NULL and both per-deal mirrors live on the deal row, the write seam keeps them consistent with the canonical participants in the same transaction (client/owner sync on set, guarded owner clear on end) — the smallest clean behavior that never lets the two diverge. The property-level seller FK is intentionally not rewritten by deal-participant maintenance (a future listing-facts story owns that surface). Stage/date/financing stay workflow-owned (CRM-14 command seams) and are not part of this maintenance story.
- Verification (SCOPED policy): new `workflow_app/tests/deal-admin.test.ts` 29/29 pass (pure contract + create/set/end branches through the fake TxRunner: atomic deal+participant creation, archived-property/client and inactive-user refusals, replace-with-invariant (one active per role), legacy mirror sync, unique-race conflict, seller leaves property facts untouched, client-end refusal, long-tail refusal). Adjacent seams green: `deal-participants` + `property-admin` + `person-admin` + `navigation-registry` 58/58; `workflow_app/tests/persistence/auth-03.test.ts` 14/14 (write inventory exactly 34, new actions gated on deal.write); `persistence/deal-participant-invariants` + `deal-participant-migration` 9/9 (live DEV invariant proofs, rolled back). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. `pnpm exec next build --webpack` passed — touched code is app-layer (server actions + client components). Full regression not run per runtime policy.
- Scope boundary: deal creation + structural participant maintenance delivered end to end. Workflow-owned deal transitions (stage/closing date/financing), the property-level seller surface, and audit rows for these verbs (AUTH-05 allow-list) are intentionally outside this story. Story stays Planned/10 on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (DEV storyboard notes update only via migration 053; no schema change). Only the OPS-05 files committed; unrelated in-flight working-tree changes (AUTH-05 actor recording) left untouched.



## 2026-08-22 — OPS-06 Intake / Resolution Administration (resolution actions)

- Role: Builder
- Story: OPS-06 — Intake / Resolution Administration. Story Board record (DEV): Partial/50, note "Needs Review read side exists. Resolution actions remain." The read side (Needs Review queue) was already delivered; this run completed the "Resolution actions" remainder — wiring the AUTH-05 actor capture that was deferred at the seam and adding the missing regression coverage.
- Files changed: `app/portal/actions.ts` (resolveIntakeAction now threads the session actor into resolveIntake; the client-supplied actor id is removed from the input contract so audit attribution is never forgeable), `db/needs-review-resolution.ts` (comment only — actor-wiring note updated), `workflow_app/tests/needs-review-resolution.test.ts` (new — 14 targeted unit tests, zero Neon), `db/migrations/051_storyboard_ops06.sql` (new — Story Board notes reconciliation for OPS-06; status/completion untouched), `docs/agent/RUNLOG.md` (this entry).
- Decision: the single resolution seam (CRM-09B, `db/needs-review-resolution.ts` — attach | create | reject with row-lock + compare-and-set idempotency, explicit-ownership refusals, canonical interaction dedupe via (source_system, source_external_id), one-transaction atomicity) was already implemented and wired into the UI; what remained was (1) the AUTH-05 runtime session wiring so `resolved_by_user_id` on the durable receipt records WHO resolved — `resolveIntakeAction` previously accepted an optional client-supplied `actorAppUserId` and passed it through, which would have made audit attribution forgeable, so it now derives the actor from the authenticated session via `portalWrite` and no longer accepts a caller-supplied id — and (2) targeted tests, since the seam had zero automated-suite coverage (only the CRM-09B `.mjs` verification script). No schema change: migration 033 already carries `resolved_by_user_id`/`resolved_at`; the read projection is untouched. The DEV board row had ONLY its notes reconciled (the human-owned board stays authoritative; status/completion not overwritten).
- Verification (SCOPED policy): new `workflow_app/tests/needs-review-resolution.test.ts` 14/14 pass (attach/create/reject happy paths, ownership-conflict refusals never silently reassigning, idempotent replay no-op, source-identity dedupe, concurrent-claim rollback via the person_identity_unique backstop, property-less general enquiry, actor capture + server-action wiring contract incl. the crm.write gate). Adjacent seams green: `person-admin.test.ts` + `property-admin.test.ts` + `navigation-registry.test.ts` 49/49; `workflow_app/tests/persistence/auth-03.test.ts` 14/14 (write-action gate contract still maps `resolveIntakeAction` → `crm.write`). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. `pnpm exec next build --webpack` passed (exit 0) — touched code is app-layer (server action). Full regression not run per runtime policy.
- Scope boundary: "Resolution actions" = the canonical resolveIntake seam + session actor capture + regression coverage, all delivered. A broader intake administration surface beyond the Needs Review queue, and any future intake channels, are separate stories. Story stays Partial on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (DEV storyboard notes update only; no schema change). Only the five OPS-06 files committed; unrelated in-flight working-tree changes (AUTH-05 actor recording) left untouched.


## 2026-08-22 — CRM-15 Closing / External SME Orchestration (SME integration seam)

- Role: Builder
- Story: CRM-15 — Closing / External SME Orchestration. Story Board record (DEV): Partial/75, note "Required branches, joins, closing date/reschedule, confirmation, close, post-close recording and terminal lifecycle proven. Real SME integrations remain." This run built the application-side orchestration seam that connects real external SMEs to the closing workflow — the "real SME integrations" remainder.
- Files changed: `workflow_app/sme-orchestration.ts` (new — CRM-15 seam), `workflow_app/task-reconciliation.ts` (SME-aware materialization), `workflow_app/tests/sme-orchestration.test.ts` (new — 11 targeted tests), `docs/agent/RUNLOG.md` (this entry).
- Decision: the workflow XML, the responsibility vocabulary, and the fact projection already existed; the missing piece was the orchestration boundary that resolves a node's `responsibility` hint to the ACTUAL responsible SME — the active canonical `deal_participant` (long tail: `role='other'` + `role_label`, e.g. appraiser/lender/title; structural: client/seller) — and materializes the engine task as a canonical task ADDRESSED TO that SME (`task.person_id`). `responsibility.ts` (Story 117 / CRM-13) already declared "the deal_participant used to find the actual responsible SME" but nothing in production used `resolveParticipantTarget`; this story wired it. No provider integration (appraiser API / lender portal) is added — that is a separately reviewed connector behind this boundary. No schema change: `task.person_id`, `deal_participant`, and `workflow_task_correlation` already support the feature (AGENTS.md: avoid schema changes when an existing abstraction supports it). An unrecorded SME never blocks the workflow: the task still materializes SME-less and reports a typed `no_participant` (never invented). Completion reuses the existing `task-completion` seam (`completeWorkflowTask`); the reconciliation caller now reads the responsibility hint from the deployed definition graph (task -> token node_id -> graph node) and orchestrates through the new seam.
- Verification (SCOPED policy): new `workflow_app/tests/sme-orchestration.test.ts` 11/11 pass (resolution matrix incl. case-insensitive long-tail and structural roles; orchestrate core attaches the SME person idempotently; no-SME fallback; two real-engine integration tests driving the RE_supermodel XML through the in-memory FakeSql — appraisal SME orchestrated + completed, workflow advances join -> closing readiness -> Closing -> closed, and an unrecorded appraiser never blocks closing). Adjacent seams green: `reconcile.test.ts` + `materialization.test.ts` + `task-completion.test.ts` + `responsibility-vocabulary.test.ts` 13/13; `re-supermodel.test.ts` 38/38 (closing lifecycle). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. No build run — only workflow_app server-side TS + tests changed; tsc covers the touched TypeScript. Live DEV read-only sanity: `listDealParticipants` SQL works and `resolveSmeParticipant` returns typed `no_participant` for a deal without an appraiser.
- Scope boundary: "real SME integrations" here means the canonical orchestration seam (resolve -> materialize addressed to the SME -> completion advances closing). Provider connectors, an SME portal/UI, and per-task SME notifications are separate stories (S-031 portal experience, DOC-04-style connectors). The story stays Partial; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (read-only DEV query only; no schema change, no migration). Only the four CRM-15 files committed; unrelated in-flight working-tree changes (AUTH-05 actor recording) left untouched.


## 2026-08-22 — CRM-18 Contact / Identity Quality (read-side hardening)

- Role: Builder
- Story: CRM-18 — Contact / Identity Quality. Story Board record (DEV): Partial/85, note "Coverage gaps, malformed identities and weak-contact diagnostics exist. Real contact import/ingestion remains." The read-only diagnostics already existed (`db/identity-quality.ts`, `/portal/identity-quality` page + component + nav); this run hardened the changed seam.
- Files changed: `db/identity-quality.ts` (determinism fix + testable seam), `workflow_app/tests/identity-quality.test.ts` (new — 5 targeted unit tests, zero Neon), `docs/agent/RUNLOG.md` (this entry).
- Decision: `identityCountByType` counted ALL `person_identity` rows including archived people's identities while every other metric in the snapshot filters `archived_at is null` — a real determinism inconsistency in a projection whose contract is "deterministic gaps". Fixed the type-count query to join `person` and filter `archived_at is null`, matching the coverage/weak-coverage/malformed queries. Also injected an optional `QueryExecutor` (repo pattern, default `sql`) so the pure derivation logic is unit-testable without Neon; the portal page call site is unchanged (`getIdentityQuality()`).
- Verification (SCOPED policy): new `workflow_app/tests/identity-quality.test.ts` 5/5 pass (coverage metrics, malformed email/phone flags via the strict normalization rules, weak-coverage filter, type distribution + structural-duplicate note, archived-exclusion SQL shape). Live DEV read-only run of `getIdentityQuality()` still returns the same snapshot after the change (4 people / 4 email / 4 phone, zero gaps on current fixtures). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. No build run — only a db module + tests changed, and tsc covers the touched TypeScript.
- Scope boundary: "real contact import/ingestion remains" is still true and intentionally NOT implemented here — the module contract explicitly forbids merges/creation/fuzzy matching/Apple Contacts, and no import/ingestion acceptance criteria are recorded on the story. The story stays Partial; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (read-only DEV queries only). No schema change. Only the two CRM-18 files committed; unrelated in-flight working-tree changes left untouched.

## 2026-08-22 — AUTH-00B Security Administration UI Foundation (reconciliation)

- Role: Builder
- Story: AUTH-00B — Security Administration UI Foundation (RECONCILIATION, intentionally Partial; record 60%).
- Files changed: `db/migrations/046_storyboard_auth00b.sql` (new — Story Board record for AUTH-00B: status Partial, completion 60, rollup true; idempotent insert, existing row keeps human status/completion), `docs/agent/RUNLOG.md` (this entry). Applied 046 to the disposable DEV branch; DEV board now shows AUTH-00B Partial/60.
- Decision: The Portal's Users/Roles/Authorities administration is not starting from zero. Inventory (verified on the DEV branch 2026-08-22): `/portal/settings` hub (+ Security Status + Break-glass panels), `/portal/settings/users`, `/portal/settings/roles`, `/portal/settings/authorities` render canonical app_user/role/authority data server-side via `db/settings-auth.ts` (getSettingsUsers/Roles/Authorities), `db/auth-status.ts` (getSecurityStatus), `lib/auth/break-glass-readiness.ts`. AUTH-02 read seams present: settings layout requires `settings.read` server-side (`app/portal/settings/layout.tsx` via resolvePortalAccess), middleware route policy maps `/portal/settings*` → `settings.read` (`lib/auth/route-policy.ts`), sidebar nav hides Settings without `settings.read` (`portal-navigation.ts`). AUTH-03: surfaces are READ-ONLY — no server action mutates users/roles/authorities; `settings.manage` exists only as a future mutation authority (`docs/auth-command-map.md`); no settings write path exists to verify. AUTH-05: durable actor/action audit exists for other admin writes (migrations 033/038/039), but no settings-management writes exist, so no settings audit trail is claimed. Explicitly outside the 60%: every-button/write-path audit of the administration UI; `settings.manage` mutations (assign roles, manage roles/authorities); enforcement claims beyond the read surfaces; AUTH-05 audit coverage of settings mutations. NOT marked Complete — unverified mutation/enforcement stays with the real AUTH stories.
- Verification: settings read projections exercised against the DEV branch (1 user / 4 roles / 9 authorities; Security Status + Break-glass readiness resolve). Targeted adjacent suites green: `workflow_app/tests/persistence/auth-02.test.ts` 23/23 and `workflow_app/tests/persistence/auth-03.test.ts` 14/14 (route policy, middleware matrix, settings.read server-side guard, nav filtering, every write action gated — settings has none). tunit- fixtures cleaned; zero leftovers. `git diff --check` clean. No TypeScript/UI code changed, so no build required under the scoped policy.
- Next action: human review of the Partial/60 record; the real AUTH stories own settings.manage mutations and enforcement audit.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (DEV branch only). No schema change (storyboard_story unchanged).

## 2026-08-21 — Autonomous worker telemetry and run lifecycle repair

- Role: Builder
- Story: Fix the autonomous coding worker so a queued story has a durable,
  observable lifecycle from queue through completion — live progress,
  heartbeat, narrative, tests summary, commit hash, terminal outcomes
  (finish/fail/cancel), and stale-run recovery.
- Files changed: `db/migrations/026_run_lifecycle_telemetry.sql` (new —
  `storyboard_story_run.updated_at` last-activity column; `Cancelled` added to
  the run result_status CHECK), `db/storyboard.ts` (run `updated_at` in type/
  mapping/selects; `finishStoryRun` APPENDS final notes to the accumulated
  narrative, sets `updated_at`, maps Cancelled run → story `Hold`; new
  `updateStoryRunProgress` — completion/timestamped milestone notes/tests
  summary; new `terminateStoryRun` — terminal Failed/Cancelled outcome with
  preserved completion + explanatory note), `db/agent-work.ts` (new
  `updateAgentWorkProgress` — progress + heartbeat via `agent_work_item.
  updated_at`; `getActiveAgentWorkItem`; `listStaleAgentWork`;
  `recoverStaleAgentWork` — marks stale Claimed/Running work terminal: run
  Failed, work Error naming last heartbeat, story Failed, queue unblocked, no
  auto-rerun; `failAgentWork` now terminates the run as Failed + story Failed;
  `cancelAgentWork` now terminates the run as Cancelled + story Hold),
  `scripts/agent-work.ts` (new `--progress`, `--cancel`, `--recover`
  subcommands; `--error` accepts completion/note/tests; claim prints
  progress/cancel hints and a stale-active-item warning), tests
  (`workflow_app/tests/agent-work.test.ts` — 8 new lifecycle tests; updated
  fakes for the CASE-parameter layouts; `storyboard-runs.test.ts` fake
  updated), and this run log.
- Decision: no new table — the existing `storyboard_story_run` columns support
  the required telemetry cleanly (`notes` accumulates an append-style
  narrative). Heartbeat = the progress update's refresh of
  `agent_work_item.updated_at` (no competing heartbeat architecture).
  Cancellation is a distinct terminal run outcome (`Cancelled`), never a
  failure; the story maps to existing canonical `Hold`. Stale recovery is
  explicit (`--recover`) and marks stale work terminal rather than silently
  rerunning potentially destructive work.
- Verification: 257/257 tests (8 new), tsc clean. Migration 026 applied to the
  shared control-plane DB (Neon HTTP driver does not execute DDL — applied via
  the WebSocket Pool over `DATABASE_URL_UNPOOLED`) and verified (`updated_at`
  present, CHECK includes Cancelled). Live end-to-end with TMP-TEL-* data: 29
  checks passed covering progress/heartbeat/narrative/tests/commit/finish/
  cancel/fail/stale-recovery/queue-unblock; all TMP data cleaned up (74
  stories, 0 work items; the single ENG-04 run is pre-existing human-directed
  history from commit 9aabca3). Next build + git diff --check follow.

## 2026-08-21 — Autonomous single-story worker scheduler

- Role: Builder
- Story: Add autonomous single-story worker scheduler — a local periodic
  wake-up for `pnpm agent:work` that removes the last manual step between a
  story becoming `Ready` and the coding agent claiming it.
- Files changed: `scripts/agent-worker-once.sh` (new — single-invocation
  wrapper: repo-root resolution with `AGENT_WORKER_REPO` override, explicit
  PATH for launchd, mkdir no-overlap lock with stale-pid recovery, invokes
  `pnpm agent:work` exactly once, timestamped start/end + exit code in
  `~/Library/Logs/CulebraLuxe/agent-worker.invocations.log`, never loops),
  `scripts/com.culebraluxe.agent-worker.plist.template` (new — LaunchAgent
  template: `StartInterval 300`, RunAtLoad, deployed wrapper outside
  TCC-protected `~/Documents`), `scripts/agent-scheduler.mjs` (new —
  `install`/`status`/`run`/`stop`/`uninstall` CLI: renders plist, deploys the
  wrapper copy, bootstraps via launchctl gui domain, kill switch = bootout +
  persisted disable), `package.json` (`agent:scheduler:install|status|run|stop|
  uninstall`), `.gitignore` (`.env.scheduler`), tests
  (`workflow_app/tests/agent-scheduler.test.ts` — 9 wrapper/plist/CLI tests,
  no DB), `docs/agent/AGENT_WORKER_SCHEDULER.md` (new — architecture, cadence,
  commands, logs, env, failure modes, emergency stop), and this run log.
- Decision: launchd user LaunchAgent over a custom daemon; the scheduler owns
  NO queue logic (DB + `pnpm agent:work` remain the authority). Discovery:
  the repo lives under TCC-protected `~/Documents`, and launchd-spawned
  processes cannot execute files there (exit 126), so `install` deploys a copy
  of the wrapper to `~/Library/Application Support/CulebraLuxe/` which the
  LaunchAgent runs with `AGENT_WORKER_REPO` pointing back at the repo.
- Verification: wrapper `no work` exit 0 + logs; queue-safety in production
  with temporary TMP-SCHED-A/B Ready stories (one claim per invocation,
  `claimed_by=scheduler`, single-worker refusal while Running, second story
  claimed only after the first finished, cleanup restored 74/0/0); install →
  RunAtLoad + kickstart scheduled runs logged (exit 0); stop persisted
  disabled and blocked runs; uninstall removed plist + deployed wrapper;
  re-install left the scheduler enabled with zero Ready stories; 9 new tests,
  full suite, tsc, Next build, git diff --check clean.
- Release: commit + push follow verification; no production migration needed.

## 2026-08-21 — Production-master agent work queue + Ready status

- Role: Builder
- Story: Add production-master agent work queue with single-worker execution —
  the dispatch layer between the authoritative Story Board and the coding agent.
- Files changed: `db/migrations/025_agent_work_queue.sql` (new — `Ready` in the
  status CHECK, `agent_work_item` table, DB-driven Ready→work-item dispatch
  trigger, one-active-per-story partial unique index, system-wide single-worker
  partial unique index), `db/agent-work.ts` (new — claim/begin/finish/fail/
  cancel repository with transactional advisory-locked `claimNextAgentWork`),
  `scripts/agent-work.ts` (new — `pnpm agent:work` one-story-at-most command,
  production-gated), `lib/storyboard-data.ts` (`Ready` status, bucket, Open
  Work view, `totalReady` model count), `components/portal/story-board.tsx`
  (Ready for execution summary stat), `app/portal/storyboard/[id]/page.tsx`
  (active work item info), `docs/agent/STORY_EXECUTION_CONTRACT.md`
  (production control plane, Ready authorization, one-item rule), tests
  (`workflow_app/tests/agent-work.test.ts` — 18 queue/lifecycle/concurrency
  tests), `docs/DEV_DATABASE.md`, and this run log.
- Decision: Execution authorization lives only on production
  `storyboard_story` (`Ready` = explicit authorization). `agent_work_item`
  carries no specification; the authoritative spec is snapshotted into
  `storyboard_story_run` at begin. The single-worker rule is enforced
  database-side (partial unique index on `(true) WHERE state IN
  ('Claimed','Running')`) and serialized by an advisory lock, so two workers
  can never race into two active stories. A work item is `Done` whenever the
  attempt finished normally and its result was recorded — `Error` is reserved
  for execution-infrastructure failure.
- Verification: DEV migration applied + full lifecycle/ordering/duplicate/
  single-worker checks with temporary TMP-* data (all passed, cleaned up);
  240/240 tests, tsc clean, Next build clean. Migration 025 promoted to
  production (74 stories preserved, history intact) and re-verified end-to-end
  with temporary TMP-PROD-* data (all passed, cleaned up). Commit/push/Vercel
  deploy to follow so DB + app land on the same released version.

## 2026-08-21 — Story execution specification + run snapshots

- Role: Builder
- Story: Finalize the Story Board as the durable shared interface (human owner /
  architect / coding agent) — execution-specification fields, immutable run
  snapshots, story-detail UI, and the global execution contract.
- Files changed: `db/migrations/024_storyboard_execution_specification.sql`
  (new — preconditions, architect_brief, context_refs, postconditions,
  architect_brief_updated_at on `storyboard_story`; six snapshot columns on
  `storyboard_story_run`), `docs/agent/STORY_EXECUTION_CONTRACT.md` (new —
  global execution rules), `lib/storyboard-data.ts` (spec fields on
  StoryRecord; total + per-workstream Complete / In Progress+Partial /
  Blocked+Failed counts), `db/storyboard.ts` (spec-field persistence;
  `architect_brief_updated_at` stamped only when the brief changes; run start
  snapshots the six spec fields; update path no longer touches system-owned
  actual_start_at / completed_at), `app/portal/storyboard/actions.ts` (spec
  fields on the form input), `components/portal/story-board.tsx` (summary
  strip + reworked workstream columns), `components/portal/write/story-board-table.tsx`
  (Inspect story detail with Overview / Dates / Product Context / Architect
  Handoff / Definition of Done / Execution History with collapsed per-run
  specification snapshots; new form fields incl. a gold-marked Architect
  brief), tests, `docs/DEV_DATABASE.md`, and this run log.
- Decision: The Story Board is the authoritative backlog with a durable
  execution specification per story. Starting a run snapshots the current
  specification so historical runs show exactly what was executed against;
  later parent edits never alter a snapshot. Human editing is scoped to §8
  fields — actual_start_at and completed_at stay system-owned (start/finish
  run lifecycle).
- Checks: Migration 024 applied to DEV; live-DEV verification passed 20/20 —
  74 stories / 0 S-* / no spec content invented for the 74 / notes preserved;
  all new fields persist through save/reload; architect_brief_updated_at
  stamped on create with a brief, unchanged on unrelated edits, re-stamped on
  brief edits; run 1 snapshots current spec; parent edit does not alter run 1
  snapshot; run 2 receives the newer spec; first actual_start_at preserved;
  Complete forces 100 + completed_at; human notes never overwritten;
  temporary TEST-SPEC-01 story + runs removed (count restored to 74);
  storyboard tests 27/27 (incl. new spec-field + snapshot + stamp tests);
  full `workflow_app` suite 140/140; `pnpm exec next build --webpack` passed;
  `git diff --check` passed.
- Database mutations: DEV branch only — migration 024 applied; a temporary
  spec/run-lifecycle story created and fully removed after verification.
  **Production untouched.**
- Decisions made: `architect_brief_updated_at` is derived in the UPDATE via
  `case when architect_brief is distinct from $N::text` (explicit `::text`
  cast required — a bare parameter in `case when $N is null` has no type
  inference under the Neon parameterized driver, found live on DEV); run
  snapshots exclude dependencies (per spec, six fields); the detail view is
  an expandable Inspect row on the existing route — no separate PM app.
- Next action: none; migrations 021–024 remain unapplied to production by
  design.

## 2026-08-21 — Story execution history + authoritative completion

- Role: Builder
- Story: Add story execution history and make completion % authoritative
- Files changed: `db/migrations/023_storyboard_execution_history.sql` (new — story dates, eight-value status CHECK, `storyboard_story_run`), `lib/storyboard-data.ts` (8 statuses, AVG(completion) rollup), `db/storyboard.ts` (dates + run lifecycle), `app/portal/storyboard/actions.ts` (date fields, run actions, Complete→100), `app/portal/storyboard/page.tsx`, `components/portal/story-board.tsx`, `components/portal/write/story-board-table.tsx` (Completion/Dates columns + expandable run history), tests, `docs/DEV_DATABASE.md`, and this run log.
- Decision: completion % becomes authoritative — workstream completion = AVG(stored completion); status is categorical (8 values) and feeds only the count buckets; runs are durable per-story execution history.
- Checks: Migration 023 applied to DEV; 74 stories / 0 S-* / all statuses in the 8 / completion 0..100 / Complete forces 100; completion-only change moved CRM 48.6→50 and Net-Net 50.6→50.9; status-only change left math unchanged; rollup=false parents excluded; full run lifecycle verified (start → In Progress + actual_start preserved; finish records result/completion/notes/commit/tests and updates the story without touching human notes; Complete sets completed_at; 2 runs preserved); `/portal/storyboard` renders Net-Net 50.6 + new columns from DEV; storyboard tests 23/23; full `workflow_app` suite 136/136; `pnpm exec next build --webpack` passed; `git diff --check` passed.
- Database mutations: DEV branch only — migration 023 applied, temporary completion/status flips applied and reverted, a temporary run-lifecycle story created and removed. **Production untouched.**
- Decisions made: status buckets (Complete / In Progress+Partial / Planned+Deferred+Hold+Failed / Blocked) feed the four count columns; run.result_status is constrained to the six outcomes (Planned and In Progress are not outcomes); run records keep the agent-reported completion while the story forces 100 on Complete.
- Next action: none; migration 023 remains unapplied to production by design.

## 2026-08-21 — Authoritative master board + DB rollups on Story Board

- Role: Builder
- Story: Replace Story Board seed with authoritative master board + DB rollups
- Files changed: `db/migrations/022_storyboard_authoritative_seed.sql` (new — completion/rollup columns + 74-story reseed), `lib/storyboard-data.ts` (8 workstreams/weights, status scoring, net-net model), `db/storyboard.ts` (completion/rollup persistence), `app/portal/storyboard/actions.ts`, `components/portal/story-board.tsx` (Net-Net + workstream rollup dashboard), `components/portal/write/story-board-table.tsx` (new fields), tests, `docs/DEV_DATABASE.md`, and this run log.
- Decision: Replace the inferred S-001…S-041 DEV rows with the human-authored 8/21 master board (74 stories), and derive workstream completion + Net-Net from persisted statuses.
- Checks: Migration 022 applied to DEV (74 rows, 0 S-*, distinct ids 74); per-workstream rollup counts reconcile with raw SQL; Net-Net 44.8%; status change CRM-14B Planned→Complete moved TXN 34.6→42.3 and Net-Net 44.8→45.9 (reverted); `/portal/storyboard` renders the board + Net-Net from DEV and persists on refresh; storyboard tests 16/16; full `workflow_app` suite green; `pnpm exec next build --webpack` passed; `git diff --check` passed.
- Database mutations: DEV branch only — columns added, rows reseeded, temporary status flips applied and reverted. **Production untouched.**
- Decisions made: rollup completion uses status scoring (COMPLETION column is stored human data but does not drive the rollup); rollup=false parent stories (CRM-14, CRM-16, PORTAL-01) are stored but excluded from counts; Deferred/Hardware-dependent fold into the open bucket so counts reconcile.
- Next action: none; migration 022 remains unapplied to production by design.

## 2026-08-21 — Story Board DEV persistence activation

- Role: Builder
- Story: Activate Story Board persistence in DEV
- Files changed: `db/storyboard.ts` (column-list fix), `db/migrations/021_storyboard_story.sql` (DEV-applied note), `docs/DEV_DATABASE.md` (new DEV setup record), and this run log. `.env.local` updated locally (gitignored).
- Decision: Create a disposable Neon branch `dev` (from `production`), point `.env.local` DEV access at it, apply migration 021 there, and verify CRUD.
- Checks: Migration applied to DEV (41 seed rows); create/edit/status/list verified against DEV via `db/storyboard.ts`; `/portal/storyboard` renders 41 stories from DEV; create → page shows → refresh persists → delete → gone; `workflow_app` suite 122/122; `pnpm exec next build --webpack` passed; `git diff --check` passed.
- Database mutations: DEV branch only — `storyboard_story` table created, 41 rows seeded, temporary fixture rows created and removed. **Production untouched.**
- Defects found: `db/storyboard.ts` interpolated a shared column-list string into queries; the Neon driver parameterizes interpolated strings (`select $1` → `?column?` rows). Fixed by writing column lists literally. (The fake-based unit test could not catch this; real-DEV verification did.)
- Next action: none; migration remains unapplied to production by design.

## 2026-08-18 — CRM-04 Lead checkpoint

- Role: Lead
- Story: CRM-04 — Website / Self-Service Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: Begin with an independent architecture gate against the corrected coordination documents before implementation.
- Checks: Repository status and `AGENTS.md` reviewed; no database or external-system operations performed.
- Result: CRM-04 is queued for architecture review.
- Fixes: None yet.
- Risks: The proposed migration is intentionally not present in Neon; implementation and fixture verification must not execute it or contact Neon.
- Next action: Run a read-only architecture review of `CURRENT.md`, `BUILDER.md`, and `REVIEWER.md`.

## 2026-08-18 — CRM-04 architecture review checkpoint

- Role: Architecture Reviewer
- Story: CRM-04 — Website / Self-Service Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: PASS; `website_intake_submission` is justified as a narrow unresolved-intake receipt and retry/idempotency boundary.
- Checks: Independent read-only review of `CURRENT.md`, `BUILDER.md`, and `REVIEWER.md`.
- Result: No Critical, High, or Medium findings. Builder phase is authorized.
- Fixes: None required. Builder must make interaction/task presentation fields deterministic and keep pre-receipt validation separate from post-claim rejection.
- Risks: Retention remains policy only; no purge job belongs in CRM-04.
- Next action: Implement the bounded Builder work order with fixture-only verification and no Neon access.

## 2026-08-18 — CRM-04 Builder checkpoint

- Role: Builder
- Story: CRM-04 — Website / Self-Service Intake
- Files changed: `app/actions/website-intake.ts`, `app/contact/page.tsx`, `components/contact.tsx`, `components/property/property-actions.tsx`, `db/migrations/006_website_intake_submission.sql`, `db/website-intake.ts`, `lib/website-intake-types.ts`, `lib/website-intake.ts`, `scripts/verify-website-intake.mjs`, and this run log.
- Decision: Implemented the approved narrow receipt boundary, pure website adapter, CRM-02/03 coordinator, atomic canonical persistence seam, property-context server action, and existing contact-path integration. Generic contact and local favorites remain unchanged.
- Checks: CRM-04 fixture verification passed with zero Neon access; CRM-01/02/03 verification suites passed; `git diff --check` passed; `pnpm exec next build --webpack` passed. Build-generated `next-env.d.ts` was restored.
- Result: Builder implementation complete and ready for independent implementation review. Migration was recorded but not executed.
- Fixes: Restricted known-transient reset behavior to an explicit error class; unexpected failures remain processing for stale-claim recovery. Receipt transitions now require ownership success before reporting completion.
- Risks: The new runtime path requires migration `006_website_intake_submission.sql` to be approved and executed separately before website intake is enabled against a database. Retention remains documented policy only, as scoped.
- Next action: Run the CRM-04 implementation review against `BUILDER.md` and `REVIEWER.md`; fix only findings rated Critical, High, or Medium.

## 2026-08-18 — CRM-04 review-fix checkpoint

- Role: Builder
- Story: CRM-04 — Website / Self-Service Intake
- Files changed: `lib/website-intake-types.ts`, `lib/website-intake.ts`, `db/website-intake.ts`, `app/actions/website-intake.ts`, `components/contact.tsx`, `app/contact/page.tsx`, `scripts/verify-website-intake.mjs`, and this run log.
- Decision: Addressed only the implementation review's Critical/High/Medium findings. Processing ownership is now an exact `processing_started_at` claim token required by every transition; no schema change was needed.
- Checks: CRM-04 fixture verification passed with zero Neon access, including stale-worker reclaim, form trust boundary, completion recovery, duplicate canonical suppression, stronger-interest SQL preservation, and transaction rejection. CRM-01/02/03 suites passed. `git diff --check` and `pnpm exec next build --webpack` passed. Build/typecheck-generated files were restored. A direct `tsc --noEmit` showed only the documented pre-existing `GuideItem` errors.
- Result: Review fixes complete; the stale worker cannot finalize after a newer worker reclaims the receipt. Contact pending state now clears through `finally`, errors are rendered safely, and the submission UUID remains stable for retry.
- Fixes: Added pure form parsing for honeypot and browser-field verification, claim-token threading and SQL predicate, behavior-focused canonical transaction fixtures, and resilient client action handling.
- Risks: The migration remains unexecuted by design. CRM-03 mixed-person race behavior continues to be covered by its unchanged green verification suite.
- Next action: Re-run independent CRM-04 implementation review.

## 2026-08-18 — CRM-04 implementation review checkpoint

- Role: Reviewer
- Story: CRM-04 — Website / Self-Service Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: PASS after bounded review fixes.
- Checks: Independent review confirmed claim-token ownership, retry-safe client state, trust-boundary fixtures, duplicate suppression, rollback coverage, and unchanged CRM-03 race coverage.
- Result: No Critical, High, or Medium findings. CRM-04 is ready for final repository verification.
- Fixes: Stale-claim ownership and rejected-action retry handling were corrected before this pass.
- Risks: Concurrency is verified through fixture behavior and SQL-contract inspection rather than live Neon, consistent with the zero-mutation constraint.
- Next action: Run the complete CRM-01–04 fixture suite, diff check, and production webpack build; then mark CRM-04 complete if green.

## 2026-08-18 — CRM-04 completion checkpoint

- Role: Lead
- Story: CRM-04 — Website / Self-Service Intake
- Files changed: No implementation changes at this checkpoint; `docs/agent/RUNLOG.md` appended.
- Decision: CRM-04 is locally complete and may advance to human migration/code review.
- Checks: CRM-01, CRM-02, CRM-03, and CRM-04 fixture suites passed with zero database access; `git diff --check` passed; `pnpm exec next build --webpack` passed.
- Result: PASS. Build-generated `next-env.d.ts` was restored.
- Fixes: No additional fixes after the passing implementation review.
- Risks: Migration `db/migrations/006_website_intake_submission.sql` is proposed and unexecuted; the runtime website intake path must not be enabled against a database until a human approves and executes it.
- Next action: Advance coordination documents to CRM-05 Email Intake architecture only.

## 2026-08-18 15:52:33 AST — CRM-05 Lead checkpoint

- Role: Lead
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, and this run log.
- Decision: Designed a provider-neutral, fixture-only email adapter POC on CRM-02/03. Provider message ID is the account-scoped idempotency identity; thread ID is correlation metadata. Direction and actor selection are deterministic, transport exclusions occur before CRM resolution, property/deal context remains exact-only, and no live provider or persistence is in scope.
- Checks: Inspected CRM-01/02/03/04 contracts, repositories, schema, migration order, and current working tree. No database/provider operation was performed.
- Result: Architecture and bounded Builder work order are ready for independent review.
- Fixes: Replaced CRM-04 coordination state only after its local PASS checkpoint; preserved CRM-04 implementation and migration unchanged.
- Risks: A future live connector needs separately reviewed durable cursor/receipt semantics before acknowledging provider delivery. Attachment media relationships and manual resolution remain deferred.
- Next action: Run a read-only CRM-05 architecture review; repair documentation findings before any implementation decision.

## 2026-08-18 — CRM-05 architecture review checkpoint

- Role: Architecture Reviewer
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: CHANGES REQUIRED; schema remains unnecessary for the fixture-only POC, but five contract boundaries and one low-risk malformed-envelope case required clarification.
- Checks: Independent read-only review of CRM-05 `CURRENT.md`, `BUILDER.md`, and `REVIEWER.md`.
- Result: No implementation authorized until canonical event types, source-token grammar, metadata admission, mailbox-role agreement, plain-text ownership, and sender cardinality are deterministic.
- Fixes: None in the reviewer role.
- Risks: An open-ended provider metadata object or ambiguous role policy could leak provider coupling or create incorrectly classified people.
- Next action: Correct only the coordination documents and return them for architecture re-review.

## 2026-08-18 — CRM-05 architecture fix checkpoint

- Role: Lead
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, and this run log.
- Decision: Inbound/outbound events now map exactly to `email_received`/`email_sent`; source tokens use a lowercase bounded grammar; metadata is constructed from an explicit transport allowlist; attachment references are opaque non-URL IDs; creation requires unanimous explicit roles across all applicable internal mailboxes; provider connectors own clean plain-text extraction; malformed sender cardinality is rejected.
- Checks: `git diff --check` passed after the documentation changes. No database, provider, environment, route, UI, or implementation operation was performed.
- Result: Architecture corrections complete and ready for independent re-review.
- Fixes: Addressed every Critical/High/Medium review concern and the Low sender-cardinality clarification without changing schema scope.
- Risks: Durable provider acknowledgement/cursor state and attachment persistence remain deliberately deferred to separately reviewed stories.
- Next action: Re-run the CRM-05 architecture review. Do not implement until PASS.

## 2026-08-18 — CRM-05 architecture completion checkpoint

- Role: Architecture Reviewer
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: PASS; no schema change is required for the provider-neutral fixture-only POC.
- Checks: Independent re-review confirmed deterministic event types, collision-safe source tokens, allowlisted metadata, opaque attachment references, unanimous mailbox-role policy, provider-owned clean-text extraction, and sender-cardinality rejection.
- Result: No Critical, High, or Medium findings. CRM-05 Builder work order is architecture-ready but intentionally unimplemented in this unattended session.
- Fixes: All prior review findings were resolved in coordination documents.
- Risks: A future live connector must add a reviewed durable receipt/cursor before acknowledgement and enforce the clean-plaintext boundary.
- Next action: Create and review preliminary CRM-06 Phone / SMS / iMessage architecture notes only.

## 2026-08-18 15:57:42 AST — CRM-06 preliminary Lead checkpoint

- Role: Lead
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/CRM-06-PRELIMINARY.md` and this run log.
- Decision: Proposed a provider-neutral, fixture-only communications adapter over CRM-02/03. Canonical identity remains strict E.164 `phone`; call/SMS/iMessage remain transport channels; event identity is provider/account scoped and transport-qualified; actor/direction selection is deterministic; call metadata and message content have separate allowlisted handling; shared/ambiguous numbers never auto-link.
- Checks: Inspected current interaction/person-identity schema, CRM-01/02/03 contracts, normalization, repository boundaries, and CRM-05 approved architecture. `git diff --check` is the required checkpoint check. No database, provider, Apple, environment, route, UI, implementation, migration, or git side effect was performed.
- Result: Preliminary architecture is ready for independent review. No implementation is authorized.
- Fixes: Preserved the approved CRM-05 coordination documents by placing CRM-06 notes in a separate durable file.
- Risks: Live delivery requires a separately reviewed provider receipt/cursor; shared/business phone ownership, number reassignment, message retention, consent, and Apple iMessage feasibility remain unresolved.
- Next action: Run a read-only architecture review of `CRM-06-PRELIMINARY.md`; do not implement CRM-06 in this unattended session.

## 2026-08-18 — CRM-06 preliminary architecture completion checkpoint

- Role: Architecture Reviewer
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/CRM-06-PRELIMINARY.md` and `docs/agent/RUNLOG.md`.
- Decision: PASS; no schema change is justified for a fixture-only preliminary POC.
- Checks: Independent re-review confirmed that webhook authenticity is separate from identity assurance, special endpoint outcomes are deterministic, identifiers/content are bounded, and persistence remains unauthorized pending policy.
- Result: No Critical, High, or Medium findings.
- Fixes: Unknown transport-observed phone actors can no longer auto-create without explicitly approved anti-spoof/ownership evidence; ambiguity and identifier rules were made deterministic.
- Risks: Provider assurance, shared-number registry ownership, durable receipt/cursor design, and jurisdiction-specific consent/retention remain pre-live gates.
- Next action: Perform the final unattended safety, fixture, diff, build, and working-tree checks.

## UNATTENDED SESSION SUMMARY — 2026-08-18

- Stories attempted: CRM-04 implementation completion, CRM-05 Email Intake architecture, CRM-06 Phone / SMS / iMessage preliminary architecture.
- Stories completed: CRM-04 reached local implementation PASS; CRM-05 reached architecture PASS; CRM-06 preliminary design reached architecture PASS.
- Architecture reviews: CRM-04 PASS; CRM-05 required one bounded correction loop and then PASS; CRM-06 required one bounded correction loop and then PASS.
- Implementation reviews: CRM-04 initially returned CHANGES REQUIRED for stale claim ownership, rejected-action retry state, and fixture gaps; those findings were fixed and the independent re-review returned PASS.
- Files changed or added during this session: `app/actions/website-intake.ts`, `app/contact/page.tsx`, `components/contact.tsx`, `components/property/property-actions.tsx`, `db/migrations/006_website_intake_submission.sql`, `db/website-intake.ts`, `lib/website-intake-types.ts`, `lib/website-intake.ts`, `scripts/verify-website-intake.mjs`, `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, `docs/agent/CRM-06-PRELIMINARY.md`, and `docs/agent/RUNLOG.md`.
- Migration proposed: `db/migrations/006_website_intake_submission.sql`.
- Migration execution: NOT EXECUTED. No migration or database statement was sent to Neon.
- Verification: CRM-01, CRM-02, CRM-03, and CRM-04 fixture suites passed with zero database access; `git diff --check` passed.
- Build: `pnpm exec next build --webpack` passed. The build's `next-env.d.ts` rewrite was restored. The build emitted only the existing Node experimental `localStorage` warning during static generation.
- Quality checks: no package/lockfile or environment changes; migration numbering is ordered through `006`; no application special-case for Casa Luar was introduced; production error logging includes only a generic error message and submission UUID, not submitted PII or secrets; fixture repositories remain injected and zero-Neon.
- Known issues: CRM-04 runtime requires human approval and separate execution of migration `006`; CRM-05 is architecture-ready but unimplemented; CRM-06 is preliminary only. Live email/phone providers remain blocked on durable receipts/cursors and provider-specific trust/retention review.
- Unresolved decisions: human approval/execution timing for migration `006`; whether/when to implement CRM-05; live-provider identity assurance; shared-number ownership; jurisdictional consent and retention.
- Recommended human review order: (1) CRM-04 migration SQL, (2) CRM-04 receipt/coordinator transaction and trust boundary, (3) contact/CTA behavior, (4) zero-Neon fixture coverage, (5) CRM-05 coordination documents, (6) CRM-06 preliminary notes.
- SQL for human inspection/execution later: `db/migrations/006_website_intake_submission.sql` only.
- Current branch: `main`.
- Exact git status at final checkpoint:
  - modified: `app/contact/page.tsx`
  - modified: `components/contact.tsx`
  - modified: `components/property/property-actions.tsx`
  - modified: `docs/agent/BUILDER.md`
  - modified: `docs/agent/CURRENT.md`
  - modified: `docs/agent/REVIEWER.md`
  - untracked: `app/actions/website-intake.ts`
  - untracked: `db/migrations/006_website_intake_submission.sql`
  - untracked: `db/website-intake.ts`
  - untracked: `docs/agent/CRM-06-PRELIMINARY.md`
  - untracked: `docs/agent/RUNLOG.md`
  - untracked: `lib/website-intake-types.ts`
  - untracked: `lib/website-intake.ts`
  - untracked: `scripts/verify-website-intake.mjs`
- Confirmation: NO PUSH. NO DEPLOY. NO NEON MUTATION. NO COMMIT. NO STAGING.

## 2026-08-18 — CRM-06 preliminary architecture review checkpoint

- Role: Architecture Reviewer
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: CHANGES REQUIRED. Webhook authenticity was incorrectly close to canonical creation evidence, special endpoint outcomes were not fully deterministic, identifier/content grammars were incomplete, and persistence needed an explicit consent/retention gate.
- Checks: Independent read-only review of `docs/agent/CRM-06-PRELIMINARY.md` against CRM-01/02/03 boundaries.
- Result: No implementation authorized pending documentation corrections.
- Fixes: None in reviewer role.
- Risks: Caller ID and provider envelopes can be spoofable or insufficient proof of ownership; live persistence without approved content policy creates privacy and consent exposure.
- Next action: Correct only the preliminary architecture and return it for re-review.

## 2026-08-18 — CRM-06 preliminary architecture fix checkpoint

- Role: Lead
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/CRM-06-PRELIMINARY.md` and this run log.
- Decision: Separated provider delivery authenticity from endpoint ownership assurance; exact existing identities may resolve, while unknown creation requires an explicitly approved anti-spoof assurance plus line-role policy. Defined deterministic outcomes for every special endpoint category, bounded case-preserving provider/correlation identifiers, normalized bounded message content, and prohibited canonical persistence until consent/retention policy approval.
- Checks: `git diff --check` required after the documentation-only corrections. No implementation, schema, migration, database, provider, Apple, environment, route, UI, commit, or push operation occurred.
- Result: All preliminary architecture review findings are addressed; ready for independent re-review.
- Fixes: Added fixture requirements for assurance, special endpoint outcomes, identifier grammar, message normalization, and the persistence gate.
- Risks: No real provider assurance capability is approved by this design; live receipt/acknowledgement, shared-number policy, and consent/retention remain future architecture decisions.
- Next action: Re-review `CRM-06-PRELIMINARY.md`; do not implement CRM-06 in this unattended session.

## 2026-08-18 — New unattended session checkpoint

- Role: Lead
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: Begin the newly authorized CRM-05 implementation sequence with a fresh architecture review of the committed coordination documents.
- Checks: Working tree was clean at session start on branch `main`, commit `d5ad6fc` (`Add website CRM intake pipeline`). Existing CRM-05 and CRM-06 documents were inspected.
- Result: CRM-05 is queued for an independent architecture gate; no implementation has begun.
- Fixes: None.
- Risks: Provider-neutral fixture scope must remain isolated from live email providers, Neon, routes, credentials, and side effects.
- Next action: Run CRM-05 architecture review and implement only after PASS.

## 2026-08-18 — CRM-05 second architecture correction checkpoint

- Role: Lead
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, and this run log.
- Decision: Separated envelope parsing from identity assurance. Exact normalized email may resolve an existing person, while unknown-person creation now requires an explicit provider-neutral `authenticated_pass` verdict plus the existing unanimous mailbox-role policy; conservative default is no creation. Defined bounded, case-preserving opaque message/thread/reply/reference IDs; an exact neutral category enum; and a connector-owned clean-plaintext contract that the adapter only NFKC-normalizes and bounds.
- Checks: `git diff --check` required after these documentation-only changes. No implementation, schema, migration, database, provider, environment, route, UI, package, commit, staging, or push operation occurred.
- Result: All current CRM-05 architecture findings are addressed; implementation remains unauthorized until independent architecture re-review returns PASS.
- Fixes: Removed envelope parsing as creation evidence, bounded and protected provider identifiers, removed any implication that the adapter verifies or strips text, and replaced open-ended provider category labels with deterministic neutral values and outcomes.
- Risks: A future connector must correctly attest the bounded authentication verdict; live ingestion still requires a separately reviewed durable receipt/cursor and provider-specific security review.
- Next action: Re-review CRM-05 architecture. Do not implement until PASS.

## 2026-08-18 — CRM-05 outbound-assurance correction checkpoint

- Role: Lead
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, and this run log.
- Decision: Restricted authenticated creation eligibility to inbound external senders. Outbound authentication proves the configured internal sender, not ownership of an external recipient; therefore an exact existing outbound recipient may resolve, but an unknown outbound recipient must return `resolution_required` and cannot auto-create in CRM-05.
- Checks: `git diff --check` required after this documentation-only correction. No implementation, schema, migration, database, provider, environment, route, UI, package, staging, commit, or push operation occurred.
- Result: The remaining High trust-boundary finding is addressed; CRM-05 is ready for another independent architecture review.
- Fixes: Added explicit Builder fixture and Reviewer coverage proving authenticated outbound internal mail cannot create an unknown external recipient.
- Risks: Auto-creation for outbound recipients remains unavailable until a separate, explicitly reviewed recipient-ownership assurance contract exists.
- Next action: Re-review CRM-05 architecture. Do not implement until PASS.

## 2026-08-18 16:47 AST — CRM-05 Builder checkpoint

- Role: Builder
- Story: CRM-05 — Email Intake
- Files changed: added `lib/crm-email-types.ts`, `lib/crm-email-normalization.ts`, `lib/crm-email-intake.ts`, and `scripts/verify-crm-email-intake.mjs`; appended this run log only. The existing Lead coordination-document changes were preserved and not rewritten by Builder.
- Decision: Implemented only the approved provider-neutral, fixture-only adapter and coordinator. Source identity is message-scoped and account-namespaced; direction, exclusions, actor assurance, mailbox-role creation policy, allowlisted metadata, opaque identifiers, attachments-as-descriptors, and exact CRM-02/03 composition are bounded explicitly.
- Checks: `verify-crm-email-intake`, CRM-04 website intake, CRM-03 person creation, CRM-02 intake, and CRM-01 foundation fixtures passed. `git diff --check` passed. `pnpm exec next build --webpack` passed; the generated `next-env.d.ts` rewrite was restored. The build emitted only the existing Node experimental `localStorage` warning during static generation.
- Result: Ready for independent implementation review. New fixtures use injected repositories, fail on unexpected creation, and performed zero Neon queries/writes and zero provider access.
- Fixes: Rejected invalid/ambiguous envelope states and source tokens; preserved case-sensitive opaque provider identifiers; separated sender authentication from envelope identity; prevented unknown outbound creation; required unanimous explicit mailbox roles for inbound creation; limited retained metadata to the approved allowlist; rejected secrets/oversize metadata and URL-like attachment IDs; preserved exact CRM context resolution.
- Risks: A future live connector still requires a separately reviewed durable receipt/cursor, provider authentication implementation, acknowledgement/retry ownership, attachment ingestion/security, and retention policy. CRM-05 deliberately contains none of those capabilities.
- Next action: Independent Reviewer should evaluate the implementation against `docs/agent/BUILDER.md` and `docs/agent/REVIEWER.md`. No live integration or persistence should proceed from this POC.
- Safety confirmation: no schema, migration, DB repository, route, UI, dependency, lockfile, environment, provider configuration, Neon mutation, external side effect, staging, commit, deploy, or push change occurred.

## 2026-08-18 — CRM-05 implementation review-fix checkpoint

- Role: Builder
- Story: CRM-05 — Email Intake
- Files changed: `lib/crm-email-normalization.ts`, `scripts/verify-crm-email-intake.mjs`, and this run log.
- Decision: Applied only the independent review findings. Attachment descriptors now require non-null string provider IDs, filenames, and MIME types plus an integer byte size. Delivery-status MIME detection now compares the exact normalized media type before parameters rather than using a prefix match.
- Checks: Added fixtures for missing/null/numeric attachment IDs and invalid descriptor primitive types; exact `multipart/report` with parameters and the non-excluding `multipart/reporting` near-match; exclusion-before-repository order; conflicting exact property hints; deal/property mismatch; and proof that property-like subject/body text performs no context lookup. The CRM-01 through CRM-05 fixture suites, `git diff --check`, and `pnpm exec next build --webpack` all passed. The build-generated `next-env.d.ts` rewrite was restored.
- Result: Review findings are resolved and the implementation is ready for independent re-review. All verification remained fixture-only with zero Neon access, zero provider access, and no canonical persistence.
- Fixes: Removed runtime primitive coercion at the attachment boundary, removed the broad MIME prefix false positive, and closed the requested exact-context and call-order fixture gaps.
- Risks: Live provider lifecycle, credentials, receipts/cursors, acknowledgements, attachment download, and persistence remain deliberately outside CRM-05.
- Next action: Independent Reviewer should re-review the bounded changes against `docs/agent/BUILDER.md` and `docs/agent/REVIEWER.md`.
- Safety confirmation: no schema, migration, database repository, route, UI, dependency, lockfile, environment, provider configuration, Neon mutation, external side effect, staging, commit, deploy, or push change occurred.

## 2026-08-18 17:09 AST — CRM-06 implementation review-fix checkpoint

- Timestamp: 2026-08-18T17:09:32-04:00
- Role: Builder
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `lib/crm-communications-normalization.ts`, `scripts/verify-crm-communications-intake.mjs`, and this run log.
- Decision: Applied only the independent review findings. Owned-line configuration now rejects every duplicate normalized phone, with a distinct error when duplicate declarations conflict on role. The fixture matrix now makes duration, idempotency, display-name, and ownership/configuration conflict boundaries explicit.
- Checks: Added negative, fractional, unsafe-integer, required, and forbidden duration cases across connected, voicemail, missed/busy, and failed call outcomes; proved correlation changes do not alter source identity; proved display names never enter identity matching; and covered duplicate/conflicting owned-line declarations. CRM-01 through CRM-06 suites passed, `git diff --check` passed, and `pnpm exec next build --webpack` passed. Build-generated `next-env.d.ts` was restored and no `tsconfig.tsbuildinfo` remains.
- Result: Review findings are resolved and the implementation is ready for independent re-review. Verification remained fixture-only with zero Neon access, provider access, or canonical writes.
- Fixes: Closed the identical-role duplicate configuration gap and expanded behavioral verification without changing the provider-neutral architecture.
- Risks: Active identity ownership conflicts remain governed by the already-green CRM-03 conflict fixtures and database identity uniqueness; CRM-06 emits exactly one canonical external phone hint and never selects among multiple actors.
- Next action: Independent Reviewer should re-review the bounded CRM-06 changes against the coordination documents.
- Safety confirmation: no schema, migration, database repository, route, UI, dependency, lockfile, environment, provider configuration, Neon mutation, external side effect, staging, commit, deploy, or push change occurred.

## 2026-08-18 — CRM-06 Lead promotion checkpoint

- Timestamp: 2026-08-18T16:52:56-04:00
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Role: Lead
- Files: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, `docs/agent/RUNLOG.md`
- Decision: Promoted the preliminary design into a bounded fixture-only architecture and Builder work order. The canonical event contract uses `call_received`, `call_placed`, `call_missed`, `sms_received`, `sms_sent`, `imessage_received`, and `imessage_sent`; strict E.164 remains the only phone identity; transport observation is separated from ownership assurance; provider event identity remains the idempotency key.
- Checks: Inspected CRM-01/02/03 and the completed CRM-05 adapter/coordinator patterns. `git diff --check` pending at this checkpoint.
- Result: Architecture is ready for independent review. No schema is needed for this POC.
- Fixes: Resolved the preliminary open call-reporting question by defining terminal received/placed/missed semantics without mirroring provider lifecycle callbacks. Defined deterministic endpoint, assurance, source-ID, content, metadata, and error outcomes.
- Risks: No real provider capability is approved for ownership assurance. Live acknowledgement/retry, consent, retention, shared numbers, number reassignment, Apple feasibility, recordings, and persistence remain deliberately deferred.
- Next action: Run an independent CRM-06 architecture review before implementation.
- Safety: No implementation, schema, database, provider, route, UI, dependency, environment, staging, commit, deploy, or push action occurred.

## 2026-08-18 — CRM-06 implementation completion checkpoint

- Role: Reviewer
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: PASS after the bounded implementation correction loop.
- Checks: Independent re-review confirmed duplicate owned-line rejection, complete call-duration matrix fixtures, correlation-ID nonidentity, display-name nonidentity, clean generated files, and zero provider/Neon/canonical writes.
- Result: No Critical, High, Medium, or Low findings. CRM-06 fixture-only implementation is locally complete.
- Fixes: Duplicate normalized owned lines now always reject; required duration, configuration, identity, and idempotency fixtures were added before PASS.
- Risks: Live transport assurance, provider delivery receipts, consent/retention, and Apple/iMessage feasibility remain deferred.
- Next action: Prepare CRM-07 WhatsApp Intake architecture only and run an independent architecture review.

## 2026-08-18 17:04 AST — CRM-06 Builder checkpoint

- Timestamp: 2026-08-18T17:04:56-04:00
- Role: Builder
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: added `lib/crm-communications-types.ts`, `lib/crm-communications-normalization.ts`, `lib/crm-communications-intake.ts`, and `scripts/verify-crm-communications-intake.mjs`; appended this run log only. Existing coordination and CRM-05 changes were preserved.
- Decision: Implemented only the approved provider-neutral, fixture-only communications adapter and coordinator. Configuration is the sole authority for owned/shared/system endpoints and creation roles; terminal call outcomes, strict-E.164 identity, assurance mapping, source identity, content, metadata, and exact context resolution follow the closed CRM-06 contract.
- Checks: CRM-01 through CRM-06 fixture suites passed. `git diff --check` passed. `pnpm exec next build --webpack` passed; generated `next-env.d.ts` and `tsconfig.tsbuildinfo` changes were restored/removed. The build emitted only the existing Node experimental `localStorage` warning during static generation.
- Result: Ready for independent implementation review. Fixtures used injected repositories and performed zero Neon queries/writes, zero provider calls, and zero canonical CRM writes.
- Fixes: Added deterministic direction and actor selection, all seven canonical event types, the closed call disposition/duration/voicemail matrix, strict endpoint and identifier validation, SMS/iMessage Unicode normalization, allowlisted metadata, duplicate-first coordinator behavior, exact CRM-02 context resolution, and CRM-03 assurance/role-gated creation.
- Risks: A future live connector still requires separately reviewed provider capability, credentials, durable receipt/cursor, acknowledgement/retry, consent/content retention, number reassignment/shared ownership policy, and Apple feasibility. None are reachable from this POC.
- Next action: Independent Reviewer should evaluate the implementation against `docs/agent/BUILDER.md` and `docs/agent/REVIEWER.md`.
- Safety confirmation: no schema, migration, database repository, route, UI, dependency, lockfile, environment, provider configuration, Neon mutation, external side effect, staging, commit, deploy, or push change occurred.

## 2026-08-18 — CRM-05 implementation completion checkpoint

- Role: Reviewer
- Story: CRM-05 — Email Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: PASS after the bounded implementation fix loop.
- Checks: Independent re-review confirmed strict attachment runtime types, exact delivery-report MIME matching, exact-context conflict coverage, no free-text linking, clean generated files, and no dependency changes.
- Result: No Critical, High, Medium, or blocking Low findings. CRM-05 implementation is locally complete.
- Fixes: Missing/null attachment identifier coercion and broad MIME-prefix exclusion were corrected before PASS.
- Risks: Live provider delivery, acknowledgement, persistence, and credentials remain deliberately absent and require future reviewed architecture.
- Next action: Promote CRM-06 preliminary notes into the active coordination files and run a fresh full architecture review.

## 2026-08-18 — CRM-06 architecture review checkpoint

- Role: Architecture Reviewer
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: CHANGES REQUIRED. The overall provider-neutral/no-schema boundary was sound, but the promoted contract left call outcomes, endpoint authority, identifier rejection, metadata keys, and intent behavior underspecified.
- Checks: Read-only review of active `CURRENT.md`, `BUILDER.md`, and `REVIEWER.md` against CRM-02/03 boundaries.
- Result: Implementation remained blocked pending bounded Lead corrections.
- Risks: Provider-event endpoint classifications or open metadata could become an accidental trust escalation; an incomplete call matrix could produce inconsistent canonical events.
- Next action: Lead must close these contracts and request re-review. No implementation authorized.

## 2026-08-18 — CRM-06 architecture review-fix checkpoint

- Timestamp: 2026-08-18T16:58:12-04:00
- Role: Lead
- Story: CRM-06 — Phone / SMS / iMessage Intake
- Files changed: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, and this run log.
- Decision: Applied only the architecture-review corrections. Added a closed call disposition/duration/voicemail matrix; made injected configuration the sole endpoint/role authority; replaced value-content secret guessing with deterministic opaque-ID rules; closed the communications metadata shape; and required no requested action with empty advisory intents.
- Checks: `git diff --check` required after this documentation-only correction.
- Result: CRM-06 is ready for architecture re-review; implementation remains unauthorized until PASS.
- Fixes: Defined collision validation across owned/shared/system configuration and allowed direct shared-sanitizer size verification without inflating transport metadata.
- Risks: Live provider assurance, receipts/acknowledgement, privacy/retention, shared identity, Apple feasibility, and persistence remain deferred.
- Next action: Independent re-review of the active CRM-06 coordination documents.
- Safety: No implementation, schema, database, provider, route, UI, dependency, environment, staging, commit, deploy, or push action occurred.
## 2026-08-18 17:11 AST — CRM-07 Lead architecture checkpoint

- Story: CRM-07 — WhatsApp Intake
- Role: Lead
- Files: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, `docs/agent/RUNLOG.md`
- Decision: Advanced the coordination documents from completed CRM-06 to a provider-neutral WhatsApp architecture. Reused strict phone identity, endpoint configuration, assurance separation, duplicate-first source identity, exact context, and injected repository boundaries. Explicitly refused to disguise WhatsApp as SMS/iMessage or add a generic message channel.
- Checks: Inspected the CRM-06 types, normalization, intake coordinator, verification coverage, current interaction-channel contract, and coordination history. `git diff --check` is pending this checkpoint append.
- Result: Architecture only. Implementation is blocked pending a separately reviewed canonical `whatsapp` channel decision; no schema change is proposed or authorized.
- Fixes: Defined provider-message idempotency, business-number authority, signed-webhook assurance limits, closed message-class metadata, attachment descriptors without fetch/persistence, privacy/retention boundaries, exact context, and no-AI/no-fuzzy rules.
- Risks: Current canonical channel cannot represent WhatsApp faithfully. Live webhook receipts, signature verification, acknowledgement/retry, credentials, consent, templates, provider media retention, and outbound delivery remain deliberately deferred.
- Next action: Independent architecture review of CRM-07 and a human decision on canonical WhatsApp channel representation before any Builder work.

## 2026-08-18 — CRM-07 architecture review checkpoint

- Role: Architecture Reviewer
- Story: CRM-07 — WhatsApp Intake
- Files changed: `docs/agent/RUNLOG.md`
- Decision: PASS for architecture only; implementation remains blocked.
- Checks: Independent review confirmed provider-neutral Cloud API boundaries, canonical phone reuse, duplicate-first source identity, configuration-owned business-number classification, assurance separation, exact context, attachment-reference privacy, and no Meta/live coupling.
- Result: No Critical, High, or Medium findings. No schema change is authorized in CRM-07.
- Fixes: None required.
- Risks: The current canonical interaction channel lacks `whatsapp`. A separately reviewed narrow channel/migration decision and exact event-type/service-message matrix are required before implementation.
- Next action: Stop CRM-07 at architecture PASS and complete the unattended quality gate.

## UNATTENDED SESSION SUMMARY — CRM-05 THROUGH CRM-07

- CRM-05 status: provider-neutral Email Intake fixture POC implemented and independently reviewed PASS.
- CRM-06 status: provider-neutral Phone / SMS / iMessage fixture POC implemented and independently reviewed PASS.
- CRM-07 status: WhatsApp architecture independently reviewed PASS; implementation blocked pending a separate canonical-channel decision.
- Architecture reviews: CRM-05 required two bounded trust-contract corrections before PASS; CRM-06 required one bounded contract correction before PASS; CRM-07 passed as architecture-only with implementation blocked.
- Implementation reviews: CRM-05 required strict attachment runtime validation, exact MIME matching, and context fixture additions before PASS. CRM-06 required duplicate owned-line rejection and matrix/idempotency/nonidentity fixture additions before PASS.
- Files changed or added: `docs/agent/CURRENT.md`, `docs/agent/BUILDER.md`, `docs/agent/REVIEWER.md`, `docs/agent/RUNLOG.md`, `lib/crm-email-types.ts`, `lib/crm-email-normalization.ts`, `lib/crm-email-intake.ts`, `scripts/verify-crm-email-intake.mjs`, `lib/crm-communications-types.ts`, `lib/crm-communications-normalization.ts`, `lib/crm-communications-intake.ts`, and `scripts/verify-crm-communications-intake.mjs`.
- Proposed migrations: none in this session.
- Migrations executed: none. Neon was not queried or mutated.
- Verification: CRM-01 through CRM-06 fixture suites passed. CRM-05/06 suites used injected fakes and performed zero provider access, zero Neon access, and zero canonical writes.
- Build: `pnpm exec next build --webpack` passed. The build-generated `next-env.d.ts` rewrite was restored; no `tsconfig.tsbuildinfo` remains. The only build warning was the existing Node experimental `localStorage` warning during static generation.
- Diff/dependencies: `git diff --check` passed. No package, lockfile, environment, schema, database repository, route, or UI changes were introduced.
- Quality findings: no adapter imports a DB repository, calls `fetch`, reads provider/environment credentials, or logs secrets. Listing names/slugs occur only in fixtures to prove exact-context and no-free-text-linking behavior; production adapters contain no property/person special cases.
- Known issues: live email and communications ingestion still need durable receipts/cursors, acknowledgement ownership, provider security review, and consent/retention policy. WhatsApp cannot be represented canonically until a separately reviewed `whatsapp` channel decision is approved.
- Human decisions required: whether to authorize a narrow interaction-channel migration for WhatsApp; exact WhatsApp event types/service-message matrix; future live-provider assurance, durable delivery, privacy, and retention policies.
- Recommended next action: review CRM-05/06 pure adapter diffs and fixtures, then open a separate architecture story for the canonical `whatsapp` channel before authorizing CRM-07 Builder work.
- Current branch: `main`.
- Exact git status:
  - modified: `docs/agent/BUILDER.md`
  - modified: `docs/agent/CURRENT.md`
  - modified: `docs/agent/REVIEWER.md`
  - modified: `docs/agent/RUNLOG.md`
  - untracked: `lib/crm-communications-intake.ts`
  - untracked: `lib/crm-communications-normalization.ts`
  - untracked: `lib/crm-communications-types.ts`
  - untracked: `lib/crm-email-intake.ts`
  - untracked: `lib/crm-email-normalization.ts`
  - untracked: `lib/crm-email-types.ts`
  - untracked: `scripts/verify-crm-communications-intake.mjs`
  - untracked: `scripts/verify-crm-email-intake.mjs`
- Confirmation: NO PUSH. NO DEPLOY. NO NEON MUTATION. NO COMMIT. NO STAGING.

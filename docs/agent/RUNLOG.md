# Agent Run Log

## 2026-08-22 — ENG-21 Isolated Worktree / Branch Execution per Worker

- Role: Builder
- Story: ENG-21 — Isolated Worktree / Branch Execution per Worker. The agent runtime (ENG-18..20) dispatched every claimed story straight into the primary checkout: the scheduled worker and the interactive architecture driver shared ONE working tree, so two executions could never run safely side by side and ownership-unknown dirty files in the primary checkout were always at risk of being swept into worker work. This run made the canonical model evidence-driven and default-on: approved integration base → story worktree/branch → worker-local changes → local commit/evidence → stop, using boring Git worktree semantics only (no custom VCS, no auto merge/rebase/push/promotion; integration stays a human checkpoint, ENG-22). The primary checkout is now never a worker scratch directory.
- Files changed: `lib/worker-workspace/types.ts` (new — `WorkerWorkspaceSpec`/`WorkerWorkspace`/`WorkerWorkspaceListItem` + `WorkerWorkspaceError`), `lib/worker-workspace/provisioner.ts` (new — unique deterministic `agent/<story>/<run>` branch + worktree OUTSIDE the primary checkout from an EXPLICIT approved base ref pinned to a fixed commit; worktreesRoot outside-checkout guard; shared-safe symlinks (node_modules/.env.local) with verified-target removal; `listWorkerWorkspaces` porcelain parse; `removeWorkerWorkspace` refuse-uncommitted + branch-always-preserved + never force; `readWorkerCommitHash` honest evidence — null when the checkout is still at the approved base; `resolveApprovedBaseRef` explicit-env-else-main fail-closed), `lib/worker-workspace/index.ts` (new — public seam), `scripts/workspace-cli.ts` (new — `pnpm agent:workspace create|status|remove|help` with injectable dispatch core for tests), `agent-runtime/types.ts` (`executionWorkspace` on the execution context — execution infrastructure only), `agent-runtime/invoker.ts` (`AgentInvokerWorkspaces` dep, `buildAgentInvokerWorkspaces` default-on env resolution, provisioning before dispatch + worktree .env.local fail-fast), `agent-runtime/deepseek/deepseek-harness-adapter.ts` (spawn cwd, session discovery, commit read and the run-narrative evidence line all use the isolated workspace; `workspaceEvidenceLine`; task-text isolation line telling the model its branch/base and never-to-push/merge/rebase), `scripts/agent-work.ts` (default-on isolated execution; `AGENT_WORKSPACE_DISABLED=1` explicit escape hatch), `scripts/agent-runtime-deepseek.ts` (interactive architecture driver wired the same), `package.json` (`agent:workspace` script), `workflow_app/tests/worker-workspace.test.ts` (new — 8 temp-repo integration proofs, zero Neon), `workflow_app/tests/workspace-cli.test.ts` (new — 6 CLI/env unit tests, zero Neon), `workflow_app/tests/evidence-summary.test.ts` (+2 ENG-21 evidence tests), `workflow_app/tests/persistence/agent-runtime-deepseek.test.ts` (+2 real-Postgres adapter proofs: isolated-worktree evidence identifies branch/worktree/base commit; no worker commit → null, never a fabricated hash), `docs/agent/WORKTREE_EXECUTION.md` (new — canonical model/ops/evidence/cleanup), `docs/agent/AGENT_WORKER_SCHEDULER.md`, `docs/agent/STORY_EXECUTION_CONTRACT.md`, `docs/workflow/MASTER_STORYBOARD.md` (Batch 6 / ENG-21 definition), `docs/workflow/STORYBOARD_STATUS.md` (PASS record + change log), `docs/agent/RUNLOG.md` (this entry).
- Decision: isolation is the OPERATING MODE, not an opt-in — `pnpm agent:work` (the one durable command, unchanged) provisions each claimed story its own `agent/<story>/<work-item-id>` branch + worktree from the approved base (explicit `AGENT_WORKSPACE_BASE_REF`, else the repo's canonical `main` branch, pinned to a commit; unresolvable ref fails closed and the claim escalates with actionable evidence — never a fake run). The DB's single-active-queue guard (migration 025) stays authoritative; the worktree mechanism is lane-ready for future concurrent workers (each work item → unique branch/path; duplicate provisioning refuses explicitly). `context.executionWorkspace` is the only new invoker→adapter seam — absent it, the legacy shared-checkout path is byte-for-byte. Cleanup is explicit (`pnpm agent:workspace remove`) and conservative: refuses any uncommitted tracked/untracked change, removes only verified provisioner symlinks, always preserves the branch; a refused removal is a no-op. Commit evidence is honest: `commit_hash` = the worker's own worktree HEAD, null when still at base; the run narrative gains `Execution workspace: branch=… worktree=… base=ref@commit`.
- Verification (SCOPED policy): new `workflow_app/tests/worker-workspace.test.ts` 8/8 pass (unique deterministic branch+worktree; TWO CONCURRENT workers coexist with no cross-contamination and the primary branch tip never advances / no remote refs — no auto-merge/push; explicit base-ref pinning incl. unresolvable-ref fail-closed and empty-ref refusal; DIRTY primary does not block creation and is never touched; honest `readWorkerCommitHash` (null at base / HEAD after worker commit / null unreadable); SAFE cleanup — refuses uncommitted work, removes only its own symlinks, preserves branch + commits; worktrees-root-outside-checkout guard; repo-root resolution from a worktree). New `workspace-cli.test.ts` 6/6 (create/status/remove dispatch contract with injected deps + `buildAgentInvokerWorkspaces` env resolution: default main, explicit base override, worktrees-root override, DISABLED=1 escape hatch). Adjacent `evidence-summary.test.ts` 10/10 (incl. `workspaceEvidenceLine` + task-text isolation line only when a workspace is provisioned). Real-Postgres `persistence/agent-runtime-deepseek.test.ts` 7/7 (incl. the two new ENG-21 isolated-workspace adapter proofs: evidence identifies branch/worktree/base commit and `commit_hash` = the worker's HEAD; no-commit run persists null). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean on the ENG-21 files. Full regression not run per runtime policy.
- Environment note: the DEV sandbox held two STALE active work items (scheduler `Running` claim for OPS-08 and a launch-guard `guard-worker` Claimed claim for PORTAL-04) that blocked every claiming suite system-wide; both were terminalized as DEV-sandbox repair via the canonical recovery/failure paths (no run existed for either; PX-25's item accidentally claimed by the suite run was restored to Ready). The pre-existing `agent-runtime-launch-guard.test.ts` FILE-run failures (its claim test races leftover Ready items for real DEV stories and leaves claims behind) reproduce identically without the ENG-21 changes — the changed invoker seam passes in isolation (name-pattern run 3/3).
- Scope boundary: the isolated worktree/branch mechanism + default-on worker/interactive wiring + evidence + CLI + docs — delivered and tested. Queue-level concurrent execution (relaxing the single-active DB guard so two lanes actually dispatch simultaneously) and human integration of worker branches into `main` (ENG-22) remain separate stories. No schema change; no migration; no production data touched.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (only the DEV sandbox was repaired as documented; no migration, no schema change, no production storyboard writes). Only the ENG-21 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert, stray temp files) left untouched.

## 2026-08-22 — INTAKE-01 Canonical Intake Message Contract

- Role: Builder
- Story: INTAKE-01 — Canonical Intake Message Contract. The realtime acquisition lane already existed (CRM-23: mac-observer → `ExternalActivityEvent` → durable integration inbox → intake stubs → `interaction.record`), but there was no canonical normalized intake message that BATCH imports and realtime observers both emit — a batch lane would have had to invent its own envelope and a second transformation stack into the CRM. This run established the ONE canonical contract: `CanonicalIntakeMessage` (source-neutral envelope carrying stable source system, source item/event id, batch/import id when relevant, occurred_at, participant/source identities, event type, raw/provenance reference, correlation/causation metadata, and only bounded source-specific payload), two lane adapters that both emit it (realtime lowers `ExternalActivityEvent`; batch lowers an `IntakeBatchManifest` + item rows), and the single projection (`toInboxInsert`) into the EXISTING durable integration inbox. The realtime processor now routes through the canonical message, so batch and realtime differ only at the edge; downstream (durable inbox → identity resolution → BusinessCommand `interaction.record`) is shared and needs no source-specific parsing.
- Files changed: `lib/intake/contracts.ts` (new — canonical envelope: `CanonicalIntakeMessage`, `intakeDedupeKey` duplicate/replay identity (source.system + source.itemId — lane/account/batch-agnostic), `intakeSourceIdentity` durable-inbox key derivation, `validateIntakeMessage`/`assertValidIntakeMessage` source-neutral validation with the 32 KB `sourcePayload` bound), `lib/intake/realtime.ts` (new — `lowerExternalActivityEventToIntakeMessage`, the realtime lane adapter output), `lib/intake/batch.ts` (new — `IntakeBatchManifest` + `lowerBatchItemToIntakeMessage`, the batch lane adapter; importId rides as correlation/provenance, never identity), `lib/intake/inbox.ts` (new — `toInboxInsert`, the ONE projection into the existing integration-inbox contract; sourcePayload never reaches it), `lib/intake/index.ts` (new — public seam), `lib/integration-inbox/processor.ts` (realtime lane now lowers through the canonical message and projects via `toInboxInsert`), `lib/integration-inbox/mapper.ts` (removed the superseded `attachmentMetadataFor`/`ContactsInteractionInput` projections — one transformation stack), `workflow_app/tests/intake-contract.test.ts` (new — 9 targeted unit tests, zero Neon), `docs/workflow/MASTER_STORYBOARD.md` + `docs/workflow/STORYBOARD_STATUS.md` (Batch 5 / INTAKE-01 definition + PASS record + change log), `docs/agent/RUNLOG.md` (this entry).
- Decision: the batch/import id is PROVENANCE/correlation, never identity — re-running an import with any import id replays instead of duplicating; the durable inbox key is derived from the canonical (system, itemId) (realtime scopes by account namespace; batch leaves it empty). Raw-source ownership stays explicit on the envelope (adapter/adapterVersion/rawReference); canonical CRM stores references only; `sourcePayload` is bounded (≤ 32 KB, matching CRM-23's metadata bound) and never reaches the inbox projection. NO new canonical CRM state model: the envelope lowers into the existing `integration_inbox` (migration 044) → existing intake stubs → existing `interaction.record` BusinessCommand. Batch-import routing into per-source intake stubs is a documented follow-on story; INTAKE-01 delivers the contract + both lane adapters + the shared inbox projection.
- Verification (SCOPED policy): new `workflow_app/tests/intake-contract.test.ts` 9/9 pass (both lanes emit the SAME canonical surface; duplicate/replay identity lane/account/batch-agnostic; durable-inbox key derivation; provenance/raw ownership with references-only persistence; equivalent batch+realtime facts project to IDENTICAL neutral inbox inserts — downstream needs no source-specific parsing; inbox insert surface is exactly the EXISTING integration-inbox contract; validation invariants + 32 KB payload bound; lib/intake contains no SQL/table — no new state model; the realtime processor's durable insert equals the canonical projection, proving convergence at the seam). Adjacent regression: `workflow_app/tests/mac-observer-inbox.test.ts` 17/17 pass — the rewired realtime path through the canonical message is byte-faithful to CRM-23 (calendar/mail/contacts completion, dedupe, poison isolation, retention). `pnpm exec tsc --noEmit` clean (exit 0). `git diff --check` clean. No `next build` run — touched code is server/lib only (no app/component surface; tsc covers the changed TypeScript). Full regression not run per runtime policy.
- Scope boundary: the canonical intake message contract + both lane adapters + the single durable-inbox projection — delivered and tested. A batch PROCESSOR (routing imported event types into the per-source intake stubs) and any new acquisition lane are follow-on stories. Story recorded PASS in the in-repo storyboard; the human board remains authoritative.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (no migration, no schema change, no Neon access). Only the INTAKE-01 files committed; unrelated in-flight working-tree changes left untouched.

## 2026-08-22 — PX-25 Managed Marketing Content

- Role: Builder
- Story: PX-25 — Managed Marketing Content. Story Board record (DEV): Ready/0, note "Move appropriate editorial content out of JSX into managed content cleanly." Sanity is retired (migration 054) and Neon is the canonical content store (the `guide_item` precedent); the primary marketing surfaces still carried their editorial copy as JSX literals. This run established the managed-content seam and moved the homepage + contact + FAQ editorial copy into it: a Neon store read server-side at request time, a canonical typed contract, and zero JSX literals left on those surfaces.
- Files changed: `db/migrations/063_marketing_content.sql` (new — `marketing_content` slots + `marketing_content_item` child rows; explicit relational columns mirroring `guide_item`; bounded kind vocabulary hero/services/culture/about/contact/faq), `db/migrations/064_seed_marketing_content.sql` (new — idempotent upsert by slot id; every string byte-identical to the JSX it replaces), `lib/marketing-content.ts` (new — canonical contract: `MARKETING_SLOTS` stable identity, `blockById`/`itemsFor` selectors, `buildHomeContent`/`buildFaqPageContent`/`buildContactPageContent` surface builders — all pure), `db/marketing-content.ts` (new — request-time repository, lazy default executor + injectable fake convention from db/storyboard.ts), `components/hero.tsx`, `components/services.tsx`, `components/culture.tsx`, `components/about.tsx`, `components/contact.tsx` (editorial literals removed; sections render from managed blocks), `app/page.tsx` (fetch + `buildHomeContent`; Hero/Services/Culture/About/Contact wired), `app/contact/page.tsx` (hero + contact block from `buildContactPageContent`; `force-dynamic`), `app/faq/page.tsx` (hero + accordion entries + CTA from `buildFaqPageContent`; `force-dynamic`), `workflow_app/tests/marketing-content.test.ts` (new — 11 targeted unit tests, zero Neon, zero React), `db/migrations/065_storyboard_px25.sql` (new — DEV Story Board notes reconciliation for PX-25; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: "managed" = Neon-backed, request-time, same house pattern as `guide_item` (Sanity retired in 054) — not a new CMS abstraction. The seed is the authoritative copy per slot (upsert + child delete/re-insert), byte-identical to the JSX it replaces so rendered output is preserved while the source of truth moves into the database. Forms and transactional UI copy stay in code: they are interface, not marketing editorial (the Contact form's submit-state messages, labels and request-type copy remain in the client component). Scope boundary: the deeper editorial pages (about/services/sellers/guide) remain JSX literals — a documented follow-up tranche on this same seam, per bounded-story discipline.
- Verification (SCOPED policy): new `workflow_app/tests/marketing-content.test.ts` 11/11 pass (contract selectors: blockById misses, itemsFor role narrowing + order preservation, buildHomeContent full/partial/empty mapping, faqEntries q/a assembly with incomplete-row dropping, buildFaqPageContent hero/entries/CTA incl. missing-list tolerance, buildContactPageContent; repository with fake QueryExecutor: blocks+items attachment in stored order, orphan item rows never leak, null columns map to null, empty tables → []). `pnpm exec tsc --noEmit` clean (exit 0). `pnpm exec next build --webpack` passed (exit 0) — touched code includes public server pages + components + lib. `git diff --check` clean. Live DEV verification: migrations 063/064 applied via `scripts/apply-migration.mjs`; `getMarketingContent` read back all 9 seeded blocks from real DEV data with items attached and special characters (·, —, ’) intact; migration 065 applied (notes-only; status/completion untouched). Full regression not run per runtime policy.
- Scope boundary: Managed Marketing Content = the Neon-backed seam + homepage (Hero/Services/Culture/About/Contact), /contact and /faq surfaces rendered from it — delivered and tested. Deeper editorial pages, images-as-media (marketing image assets remain static `/images/*` paths), and an edit UI for the content tables are NOT part of this run. Story status/completion remain the human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migrations 063/064/065 applied to the DEV branch only; production story-board run telemetry left untouched per this command's policy). Only the PX-25 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert, stray temp files) left untouched.

## 2026-08-22 — PORTAL-04 Final Portal Visual Polish

- Role: Builder
- Story: PORTAL-04 — Final Portal Visual Polish. Story Board record (DEV): Planned/0, note "Final presentation-level graphics, spacing and visual hierarchy after core data/write surfaces settle." PORTAL-02 (Navigation IA) and PORTAL-03 (Lisa Demo Polish) already made Dashboard → Attention → Dossier → Deal → Activity read as one coherent application; the core data/write surfaces (OPS-02..06, CRM-11/13/18, workflow seams, DOC-06/07) have since settled. This run delivered the final presentation layer on top: one canonical page head with a gold brand hairline across the coherent NEXUS flow, the gold monogram seal in the operating shell, the gold accent rule on the dashboard headline metrics, and a refined brand row. Pure presentation work — no data, schema, routing, authority, or behavior changes.
- Files changed: `components/portal/page-header.tsx` (new — the canonical Portal page head: short gold hairline, eyebrow, serif title, optional subtitle and an optional trailing slot; geometry preserved exactly from the inline heads it replaces and trailing children keep their own margins, so detail-page meta rows and back links render at their established spacing), `components/portal/dashboard.tsx` (page head → `PageHeader`; the four headline `MetricCard`s gain a gold accent rule across the top — `overflow-hidden` card with a `h-0.5 bg-[#c6a15b]` band and unchanged inner padding), `components/portal/attention.tsx`, `components/portal/deals-portfolio.tsx`, `components/portal/client-manager.tsx` (both empty-state and main heads, "New client" button kept as the trailing slot), `components/portal/activity-feed.tsx`, `components/portal/showings.tsx`, `components/portal/relationship-dossier.tsx` (empty-state head + the person head with its role/status/location meta row and back link as the trailing slot), `components/portal/deal-workspace.tsx` (empty-state head + the deal head with its stage pill/location and back link as the trailing slot), `components/portal/operating-shell.tsx` (brand row: gold serif "C" monogram seal in a `border-[#c6a15b]/60` square beside the wordmark, hover-refined — presentation-level graphics in the always-visible chrome), `db/migrations/062_storyboard_portal04.sql` (new — DEV Story Board notes reconciliation for PORTAL-04; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: PORTAL-04 is presentation-only and bounded to the coherent NEXUS application flow (Dashboard → Attention → Deals → Clients → Activity → Showings + the Dossier and Deal Workspace detail heads) plus the always-visible operating shell and the dashboard headline metrics. The shared `PageHeader` is the single source of truth for page-head spacing/typography so the primary surfaces cannot drift apart again; the gold hairline and monogram are the "presentation-level graphics" the story names, drawn from the brand gold (`#c6a15b`) that already marks the active operating-surface tab. The OPS/TECH/SUPPORT admin surfaces keep their existing (identical-class) inline heads without the hairline — primary surfaces carry the brand accent, admin surfaces stay quiet; extending the accent there is a deliberate later decision, not part of this story.
- Verification (SCOPED policy): directly adjacent `workflow_app/tests/navigation-registry.test.ts` 5/5 pass (the shell renders from the navigation registry — unchanged). `pnpm exec tsc --noEmit` clean (exit 0, baseline confirmed clean before edits). `pnpm exec next build --webpack` passed (exit 0) — touched code includes server + client portal components. `git diff --check` clean. Full regression not run per runtime policy.
- Scope boundary: Final Portal Visual Polish = the canonical page head + gold accents on the NEXUS flow, shell and dashboard metrics — delivered. No typography/geometry change to any body surface, no layout-system change, no interaction change, and no change to the OPS/TECH/SUPPORT surfaces (documented above). Story stays Planned on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 062 recorded but NOT applied to any database — the runtime policy forbids mutating production data/schema this command; apply on review). Only the PORTAL-04 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert, OPS-08 storyboard follow-up, stray temp files) left untouched.

## 2026-08-22 — OPS-08 Story Board Batch / Next Work Selection (Next 20)

- Role: Builder
- Story: OPS-08 — Story Board Batch / Next Work Selection. Story Board record (DEV): Planned/10, note "Add bounded work-selection capability such as Next 20 without building Jira." The board already persisted the authoritative 74-story backlog with create/edit/status and execution runs (OPS-07 + migration 021..024), and every story already carried an implementation `batch` integer, but nothing derived "what should the agent pick up next" — the operator scanned the whole board. This run delivered the bounded work-selection capability as a PURE, deterministic projection over the stored stories: no Jira, no assignments, no sprints, no new tables, no schema change.
- Files changed: `lib/storyboard-data.ts` (OPS-08 selection contract: `priorityRankOf` — the ENG-16 priority ladder, documented as the canonical rank for the vocabulary module; `isStoryActionable` — rollup=true AND status ∈ {Planned, Ready, In Progress, Partial}; `dependencyStoryIds` — story-ID-like token extraction from the free-text dependencies field, case-insensitive, deduped; `selectNextWork(stories, {limit?})` — eligibility = actionable AND every board story referenced in `dependencies` is Complete (references to stories the board does not know are unverifiable and never block), ordering = batch ascending (unbatched last) → priority rank → planned start (earliest first, unplanned last) → id, capped at `NEXT_WORK_DEFAULT_LIMIT` (20, max `NEXT_WORK_MAX_LIMIT` 50) with `entries` (rank 1-based), `totalEligible`, `totalBlockedByDependency`, `limit` and `truncated`; the input array is never mutated), `components/portal/story-board.tsx` (new server-rendered "Next Work — Next 20" section between the executive dashboard and the controls: ranked entries with ID link, title, workstream, batch badge, priority and status pills, plus an honest footer explaining the eligibility/ordering contract and the held-back-by-dependency count), `app/portal/storyboard/page.tsx` (computes `selectNextWork(stories)` from the freshly loaded board and passes it to the board view), `workflow_app/tests/next-work.test.ts` (new — 16 targeted unit tests, zero Neon, zero React), `db/migrations/061_storyboard_ops08.sql` (new — DEV Story Board notes reconciliation for OPS-08; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: the "Next 20" capability is a derived projection, not a second backlog — the board stays the single source of truth and `selectNextWork` is a pure function of the stored rows, so the selection can never drift from what the board actually says (AGENTS.md: extend existing abstractions; avoid schema changes when an existing abstraction supports the feature — here the existing `batch` field is the batch axis and the existing `dependencies` free text is the dependency gate). Eligibility deliberately excludes reference/parent rows (rollup=false — their children carry the weight) and non-actionable statuses; ordering deliberately ranks batch before priority so a human-batched implementation wave is honored, with priority/planned-start/id as the deterministic tiebreaks inside a batch. The dependency gate only trusts stories the board itself knows: an unverifiable reference (typo, future id, external system) never silently stalls the queue.
- Verification (SCOPED policy): new `workflow_app/tests/next-work.test.ts` 16/16 pass (priority ladder incl. unknown→99; eligibility matrix over all nine statuses + rollup=false exclusion; dependency-token extraction incl. dedupe/case-insensitivity and non-story tokens like dates; default cap 20 with 25-story truncation + sequential 1-based ranks; cap clamping 1..50 and NaN/undefined defaulting; priority ordering incl. unknown last; batch-before-priority ordering with unbatched last; planned-start tiebreak with unplanned last; id tiebreak; dependency gate incl. unknown-reference non-blocking and Complete-unblocks; determinism + input immutability; empty board) plus directly adjacent storyboard seams `storyboard-filter.test.ts` + `storyboard-rollup.test.ts` + `storyboard.test.ts` 42/42. `pnpm exec tsc --noEmit` clean (exit 0). `pnpm exec next build --webpack` passed — touched code includes a portal server component + lib + page. `git diff --check` clean. Pre-existing-at-HEAD `storyboard-runs.test.ts` 2 fails reproduced identically in a clean worktree at d805a86 (run-snapshot fake drift, unrelated to OPS-08) — left untouched per scope discipline. Full regression not run per runtime policy.
- Scope boundary: Next Work Selection = the bounded, deterministic projection + its board section — delivered and tested. Real Jira-style features (assignments, sprints, per-operator queues, scheduling) and any write-side "claim next" action are deliberately NOT built; execution control remains the existing start/finish-run lifecycle and the human-owned board. Story stays Planned on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 061 applied to the DEV branch only; notes-only — no schema change, no business data; production story-board run telemetry left untouched per this command's policy). Only the OPS-08 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert, stray temp files) left untouched.

## 2026-08-22 — PX-24 Buyers Search / Filter 2.0 (contract evolution)

- Role: Builder
- Story: PX-24 — Buyers Search / Filter 2.0. Story Board record (DEV): Planned/0, note "Improve query/search contract as inventory expands." The server-side filter contract (PX-24B: `db/properties.ts` `getFilteredProperties`) and the URL-as-source-of-truth showroom reconciliation (PX-24C) shipped with the Public Buyer V1 commit (`e994c29`), but the client side carried a drifted inline implementation — the showroom re-implemented the free-text haystack and the sort switch that `lib/search-contract.ts` already owned, with zero automated coverage. This run completed the contract evolution: the showroom now consumes the canonical contract, the URL round trip gained its inverse parser, and the pipeline gained its first dedicated suite.
- Files changed: `lib/search-contract.ts` (PX-24 evolution: new `sortProperties` — the canonical featured/price-high/price-low/name ordering extracted from the showroom, returns a new array and never mutates input; new `applySearchFilters` — match on the SAME matcher as the saved-search alerts, then order canonically, one step, idempotent over the already server-filtered list so the client can never contradict the server; new `searchParamsToFilters` — the inverse of `searchFiltersToQuery`, so the URL round trip has ONE parse rule with the same category/sort defaulting as the showroom), `components/buyers-property-showroom.tsx` (controls typed with the canonical `SearchCategory`/`SearchSort`, URL reconciliation via `searchParamsToFilters` (PX-24C), URL push via `searchFiltersToQuery`, list rendering via `applySearchFilters`; the dead `matchesCategory` helper and the duplicated haystack/sort switch are gone — the component sheds ~75 lines), `app/buyers/page.tsx` (initial filter seed derived through the same canonical `searchParamsToFilters` parser so page seed and every later reconcile agree), `workflow_app/tests/search-filter-2.test.ts` (new — 24 targeted unit tests, zero Neon, zero React), `db/migrations/059_storyboard_px24.sql` (new — DEV Story Board notes reconciliation for PX-24; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: the server stays the AUTHORITATIVE filter for what renders (PX-24B) — this run unified the CLIENT-side mirror instead of moving filtering: one matcher (`matchesSearchFilters`) now powers both the inventory list and the saved-search alert counts, one parse rule (`searchParamsToFilters`/`searchFiltersToQuery`) powers both the page seed and the showroom's URL reconciliation, and one ordering (`sortProperties`) replaces the inline switch. Applying `applySearchFilters` over the server-filtered props is idempotent by construction and makes narrow filter changes feel instant while the server round trip completes. No schema change, no listing-specific hardcoding, no server-contract change (AGENTS.md).
- Verification (SCOPED policy): new `workflow_app/tests/search-filter-2.test.ts` 24/24 pass (sort matrix: featured/price-high/price-low/name incl. null-prices-last, featured-first, input-array mutation safety, sort-is-ordering-not-filter; filter matrix: category, price cap incl. price-less exclusion, beds floor with land excluded mirroring the server SQL, case-insensitive view/q over the canonical haystack, combined narrowing, empty inventory; idempotence over an already-filtered list proving server+client converge; URL round-trip losslessness filters→query→filters and stability query→filters→query — the push-guard contract — plus whitespace self-clean) plus directly adjacent `saved-searches.test.ts` (same contract module) + `compare.test.ts` + `favorites.test.ts` 53/53 — 77/77 total. `pnpm exec tsc --noEmit` clean (exit 0). `pnpm exec next build --webpack` passed (exit 0) — touched code includes public client components + lib. `git diff --check` clean. Live DEV verification: migration 059 applied cleanly via `scripts/apply-migration.mjs` — the PX-24 row now carries the delivered-outcome note with status (`Planned`) and completion (0) untouched (read-back verified). Full regression not run per runtime policy.
- Scope boundary: Search / Filter 2.0 here = the canonical client-side search/filter/order contract consumed by the showroom, with the server remaining authoritative — delivered and tested. New filter dimensions (bathrooms, amenities, property sub-types) and moving structured filtering fully client-side over the full inventory are NOT part of this run; the PX-24B architecture decision (structured filters in SQL) stands. Story stays Planned on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 059 applied to the DEV branch only; notes-only — no schema change, no business data; production story-board run telemetry left untouched per this command's policy). Only the PX-24 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert, stray temp files) left untouched.

## 2026-08-22 — PX-23 Saved Searches + Alerts (browser-local V1)

- Role: Builder
- Story: PX-23 — Saved Searches + Alerts. Story Board record (DEV): Planned/0, note "Search + identity + notification integration." The search surface is `/buyers` (server-filtered inventory via `getFilteredProperties`); nothing existed to save a search or alert on it. This run delivered the browser-local V1 to parity with the PX-21/PX-22 seams: save the current filter state, apply it back through the server-filtered URL, and an honest "N new matches since you last viewed" alert computed over the live inventory.
- Files changed: `lib/search-contract.ts` (new — canonical buyers search contract: `SearchFilters` shape mirroring the showroom/URL control state and the server `PropertyFilterInput`, `searchFiltersToKey` canonical order-independent dedupe signature, `searchFiltersToQuery` reproducing the showroom's URL-push contract exactly, `describeSearchFilters` human label from non-default parts, and `matchesSearchFilters` — a pure client-side alert matcher mirroring the server filter SQL; the server stays authoritative for what renders), `lib/saved-searches.ts` (new — pure browser-local storage seam: `SavedSearch` = id/name/filters/createdAt/lastCheckedAt/lastMatchIds, save/refresh deduped by filter signature (re-save = "watching from now"), `removeSavedSearch`, `markSavedSearchViewed`, `newMatchIds` honest seen-diff, `SAVED_SEARCHES_CHANGED_EVENT` dispatched from `writeSavedSearches` with a no-op-write guard, and `saveSearch` re-reads storage so the reported `created` flag always agrees with what was persisted), `components/property/saved-searches-panel.tsx` (new — Save-this-search control + saved-searches chips with per-search match counts, gold "+N new" alert badges, apply (navigates to the server-filtered /buyers URL and marks the search viewed so the alert clears), and remove), `components/buyers-property-showroom.tsx` (panel wired below the sticky filter bar; `initial` prop typed as `SearchFilters`), `workflow_app/tests/saved-searches.test.ts` (new — 23 targeted unit tests, zero Neon, zero React), `db/migrations/058_storyboard_px23.sql` (new — DEV Story Board notes reconciliation for PX-23; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: browser-local V1, no schema change, no server-state change (AGENTS.md: avoid schema changes when an existing abstraction supports the feature — here the browser-local seam pattern from PX-21/PX-22 and the existing live inventory passed to the showroom support it entirely). Alerts are an honest seen-diff: each search remembers the canonical property ids it was last viewed against; the alert count = current matches (client-side matcher over the freshly fetched live inventory) minus that seen set, and applying a search marks it viewed. Free-text/price/beds/view matching mirrors the server SQL contract exactly (including view words in the haystack and land-excluded beds), while the server remains the authoritative filter for rendering — the client matcher exists only to count alerts without a per-search server round trip. `sort` deliberately never affects matching. No listing-specific hardcoding.
- Verification (SCOPED policy): new `workflow_app/tests/saved-searches.test.ts` 23/23 pass (matcher matrix: category/price/beds/view/q incl. land exclusions and case-insensitivity, sort-ignored; canonical key across construction/whitespace/case; apply-URL contract; describe labels; storage: empty/corrupted/malformed, no-op-write guard, save create + dedupe-by-signature refresh, distinct filters distinct entries, remove, mark-viewed, new-match diff incl. price-drop-newly-matching and clear-after-view, event-listener convergence, private-mode containment, no-window safety) plus directly adjacent `favorites.test.ts` + `compare.test.ts` 30/30 — 53/53 total. `pnpm exec tsc --noEmit` clean (exit 0). `pnpm exec next build --webpack` passed (exit 0) — touched code includes public client components + lib. `git diff --check` clean. Live DEV verification: migration 058 applied cleanly via `scripts/apply-migration.mjs` — the PX-23 row now carries the delivered-outcome note with status (`Planned`) and completion (0) untouched (read-back verified). Full regression not run per runtime policy.
- Scope boundary: Saved Searches + Alerts V1 = save current /buyers filter state (deduped by signature), apply through the server-filtered URL, remove, and honest per-search "new matches since you last viewed" badges on /buyers — delivered and tested. Identity/CRM-backed persistence (server-side saved searches per authenticated user) and real notification delivery (email/push when a saved search gains matches) remain the documented open scope; their prerequisites are tracked under PORTAL-01 / AUTH-01..05 (auth is Blocked/Planned on the board). Story stays Planned on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 058 applied to the DEV branch only; notes-only — no schema change, no business data; production story-board run telemetry left untouched per this command's policy). Only the PX-23 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert) left untouched.

## 2026-08-22 — PX-22 Favorites (browser-local V1 hardening)

- Role: Builder
- Story: PX-22 — Favorites. Story Board record (DEV): Partial/50, note "Browser-local implementation exists. Identity/CRM-backed persistence remains." The browser-local favorites V1 (heart toggles + `/favorites` page) existed but was rough: storage logic embedded in the `.tsx` component with no automated coverage, hearts had no cross-component sync (a save on one surface would not update hearts elsewhere), the saved-properties page could not remove an entry (only un-hearting on the property page worked) and never pruned delisted listings, and the page was orphaned (no header/footer link). This run hardened the seam to parity with the compare V1 (PX-21) and made the feature discoverable.
- Files changed: `lib/favorites.ts` (new — pure browser-local storage seam: canonical-id keyed, deduped, backward compatible with the legacy array-of-ids format, `FAVORITES_CHANGED_EVENT` dispatched from `writeFavorites` with a no-op-write guard so refresh/prune listeners converge instead of re-triggering, `toggleFavorite` re-reads storage after the write so the reported state always matches what was persisted, `removeFavorite(id)`, `pruneFavorites(validIds, validSlugs)`), `components/property/save-property.tsx` (hearts now read `lib/favorites.ts` and subscribe to the change event so every heart on the site stays in sync), `components/property/favorites-view.tsx` (the saved-properties page live-updates on the change event, prunes stale entries against the live public set by canonical id/slug, and gains a per-card remove control), `components/property/favorites-link.tsx` (new — header link to `/favorites` with a live saved-count badge subscribing to the change event), `components/site-header.tsx` (Saved link with count in the desktop nav and mobile menu), `workflow_app/tests/favorites.test.ts` (new — 15 targeted unit tests, zero Neon, zero React), `db/migrations/057_storyboard_px22.sql` (new — DEV Story Board notes reconciliation for PX-22; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: favorites remain browser-local (no schema, no server state, no listing-specific hardcoding — AGENTS.md). The seam mirrors `lib/compare.ts` exactly: canonical `property.id` is the stable identity, entry matching falls back to slug for legacy rows, stale entries are pruned against the live public set before rendering, and cross-component sync is a custom window event dispatched only on real persistence changes (no-op guard prevents listener loops). The existing storage key (`culebraluxe:saved-properties`) is unchanged so previously saved hearts survive.
- Verification (SCOPED policy): new `workflow_app/tests/favorites.test.ts` 15/15 pass (empty/corrupted/malformed storage, legacy array-of-ids backward compatibility, no-op write without dispatch, toggle add/remove/dedupe by id, isFavorite, removeFavorite, stale prune by id and slug incl. full-clear, private-mode containment, event-listener convergence, no-window safety) plus directly adjacent `workflow_app/tests/compare.test.ts` 15/15 — 30/30 total. `pnpm exec tsc --noEmit` clean (exit 0; stale `.next/types` duplicates removed first). `pnpm exec next build --webpack` passed (exit 0) — touched code includes public client components + lib. `git diff --check` clean. Live DEV verification: migration 057 applied cleanly via `scripts/apply-migration.mjs` — the PX-22 row now carries the delivered-outcome note with status/completion untouched (read-back verified). Full regression not run per runtime policy.
- Scope boundary: Favorites V1 = hearts (property page, buyers showroom, home carousel) + discoverable saved-properties page with live sync, stale prune, remove control, and header count — delivered and tested. Identity/CRM-backed persistence remains the documented open scope; its prerequisites are tracked under PORTAL-01 / AUTH-01..05 (auth is Blocked/Planned on the board). Story stays Partial on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 057 applied to the DEV branch only; notes-only — no schema change, no business data; production story-board run telemetry left untouched per this command's policy). Only the PX-22 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert) left untouched.

## 2026-08-22 — PX-21 Compare Properties (public buyers surface)

- Role: Builder
- Story: PX-21 — Compare Properties. Story Board record (DEV): Planned/0, note "Strong fit with canonical structured property facts." The browser-local compare V1 (per-card toggle + side-by-side facts table on `/buyers`) shipped with the Public Buyer V1 commit (`e994c29`) but had zero automated coverage, no live sync between the toggle buttons and the table, and an honest-capacity defect. This run completed the story: hardened the storage seam, made the table react to selection changes, and added the first targeted suite.
- Files changed: `lib/compare.ts` (capacity semantics: adding past `COMPARE_MAX` is now rejected — never a silent drop of the newest entry — and `toggleCompare` re-reads storage after the write so the reported state always matches what was persisted; new `removeCompare(id)`; new `COMPARE_CHANGED_EVENT` dispatched from `writeCompare` with a no-op-write guard so prune/refresh listener loops converge instead of re-triggering), `components/property/compare-bar.tsx` (live refresh on the compare-changed event so the section appears/updates as the user toggles cards; canonical hero image in each header cell with graceful fallback; per-row remove control), `components/property/compare-property.tsx` (card toggle now listens for compare-changed events so it stays in sync with table removals), `workflow_app/tests/compare.test.ts` (new — 15 targeted unit tests, zero Neon, zero React), `db/migrations/056_storyboard_px21.sql` (new — DEV Story Board notes reconciliation for PX-21; status/completion untouched — human board decision), `docs/agent/RUNLOG.md` (this entry).
- Decision: the compare seam is browser-local and bounded (3 slots, dedupe by canonical property id, stale entries pruned against the live public set) — no schema, no server state, no listing-specific hardcoding (AGENTS.md). Cross-component sync is a custom window event dispatched only on real persistence changes; the no-op guard compares the serialized value before writing so listener refreshes converge. The table compares canonical structured property facts only (`db/properties.ts` `PropertySummary` is unchanged) and honors the property UI contract (land hides beds/baths, missing values render as em-dash, "Price Upon Request").
- Verification (SCOPED policy): new `workflow_app/tests/compare.test.ts` 15/15 pass (empty/corrupted/malformed storage, write bounds, no-op write without dispatch, toggle add/remove/dedupe, capacity rejection without dropping the newest entry, removeCompare, stale prune incl. slot freeing, private-mode containment, event-listener convergence, no-window safety). `pnpm exec tsc --noEmit` clean (exit 0). `pnpm exec next build --webpack` passed (exit 0) — touched code includes public client components + lib. `git diff --check` clean. Live DEV verification: migration 056 applied cleanly via `scripts/apply-migration.mjs` — the PX-21 row now carries the delivered-outcome note with status/completion untouched (read-back verified). Full regression not run per runtime policy.
- Scope boundary: compare V1 = toggle + bounded selection + side-by-side canonical facts table, delivered and tested. A dedicated `/compare` route, compare from the property detail page, and identity/CRM-backed persistence are not part of this story (PX-22-style persistence belongs to its own story). Story stays Planned on the board; completion is a human board decision.
- Confirmation: NO PUSH. NO PRODUCTION MUTATION (migration 056 applied to the DEV branch only; notes-only — no schema change, no business data). Only the six PX-21 files committed; unrelated in-flight working-tree changes (AUTH-05 receipt-actor edits, SB-01 storyboard revert) left untouched.

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

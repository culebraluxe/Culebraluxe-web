-- CulebraLuxe Portal
-- Story Board — durable storyboard story table + initial seed
-- Migration: 021_storyboard_story.sql
--
-- /portal/storyboard reads and edits these rows. Story IDs are human-assigned
-- (e.g. CRM-19, OPS-07, PX-27) and are the primary key; they are never
-- auto-generated. The initial 41 rows are the existing human-authored backlog
-- transcribed from docs/workflow/MASTER_STORYBOARD.md and
-- docs/workflow/STORYBOARD_STATUS.md (seed 2026-08-20).
--
-- NOT executed. Apply manually after review.

begin;

create table if not exists storyboard_story (
    id text primary key,
    workstream text not null,
    title text not null,
    priority text not null,
    status text not null,
    notes text not null default '',
    batch integer,
    goal text,
    scope text,
    acceptance_criteria text,
    dependencies text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- One-time seed of the existing 41 storyboard records (idempotent).
insert into storyboard_story
    (id, workstream, title, priority, status, notes, batch)
values
  ('S-001', 'CRM / Intake', 'CRM-01 Source-Idempotent Interaction Inputs', 'High', 'Complete', 'Canonical (source_system, source_external_id) idempotency with database uniqueness backstop (migration 005).', 1),
  ('S-002', 'CRM / Intake', 'CRM-02 Neutral Inbound Events, Identity Normalization & Context Resolution', 'High', 'Complete', 'Strict E.164 / canonical identity; exact trusted context only; advisory intents, no action inference.', 1),
  ('S-003', 'CRM / Intake', 'CRM-03 Explicitly Authorized Person Creation', 'High', 'Complete', 'Atomic creation, existing-person-wins race recovery, source-token validation.', 1),
  ('S-004', 'CRM / Intake', 'CRM-04 Website / Self-Service Intake', 'High', 'Complete', 'Canonical website intake pipeline (migration 006 recorded); Neon execution is operational.', 1),
  ('S-005', 'CRM / Intake', 'CRM-05 Provider-Neutral Email Intake', 'Medium', 'Complete', 'Fixture POC reviewed PASS; live provider ingestion deferred to separate stories.', 1),
  ('S-006', 'CRM / Intake', 'CRM-06 Phone / SMS / iMessage Communications Intake', 'Medium', 'Complete', 'Fixture POC reviewed PASS; live provider ingestion deferred to separate stories.', 1),
  ('S-007', 'CRM / Intake', 'CRM-07 WhatsApp Intake Architecture (Architecture Only)', 'Medium', 'Readiness PASS', 'Architecture reviewed PASS; implementation blocked on S-008.', 1),
  ('S-008', 'CRM / Intake', 'Canonical WhatsApp Interaction Channel Decision', 'High', 'Planned', 'Decision pending; narrow whatsapp channel addition recommended; unblocks S-009.', 1),
  ('S-009', 'CRM / Intake', 'WhatsApp Provider Connector Implementation', 'Medium', 'Blocked', 'Blocked on S-008; only provider-neutral scaffolding exists (lib/crm-whatsapp-*).', 1),
  ('S-010', 'CRM / Intake', 'V1 DB Unblock M-1: WhatsApp Interaction Channel (migration 010)', 'Medium', 'Complete', 'Channel recorded in schema; actors resolve via canonical phone; use gated by S-008.', 2),
  ('S-011', 'CRM / Intake', 'V1 DB Unblock M-2: General Enquiry Website Intake (migration 011)', 'Medium', 'Complete', 'Generic /contact flows through the canonical pipeline; property-scoped intake unchanged.', 2),
  ('S-012', 'Portal / Operations', 'V1 DB Unblock M-3: Deal Participants (migration 012)', 'Medium', 'Read-side complete', 'Migration and read models in place; legacy FKs remain source of truth; FK migration is later.', 2),
  ('S-013', 'Portal / Operations', 'V1 DB Unblock M-4: Showing Lifecycle (migration 013)', 'Medium', 'Read-side complete', 'Migration and read models in place; showing→interaction write behavior is a later story.', 2),
  ('S-014', 'Portal / Operations', 'V1 DB Unblock M-5: Offer Model (migration 014)', 'Medium', 'Read-side complete', 'Migration and read models in place; deal.offer_price untouched; no backfill.', 2),
  ('S-015', 'Portal / Operations', 'AUTH-01 Auth & Security Model Foundation', 'High', 'Complete', 'Schema foundation plus docs; runtime enforcement not yet active (see S-016).', 2),
  ('S-016', 'Portal / Operations', 'AUTH-02 Auth Runtime Enforcement Activation', 'Critical', 'Blocked', 'Blocked on human bootstrap order: provider login, subject link, break-glass proof.', 2),
  ('S-031', 'Portal / Operations', 'Portal Workflows Experience', 'Medium-High', 'Open', 'Read-only summaries exist; task/deadline/action UX not built (held by no-UI constraint).', 4),
  ('S-032', 'Portal / Operations', 'CRM-14 Closing Orchestration', 'Critical', 'Deferred', 'Deferred from V1 unblock; closing readiness, date reschedule, post-close recording.', 4),
  ('S-017', 'Platform / Engineering / Data', 'WF-01 Workflow Engine Preservation & Architecture Assessment', 'High', 'Complete', 'Read-only archaeology; engine preserved generic and domain-neutral.', 3),
  ('S-018', 'Platform / Engineering / Data', 'WF-02 Ogden Integration Seam (Application-Side Contracts)', 'High', 'Complete', 'lib/workflow contracts; the application never imports the engine.', 3),
  ('S-019', 'Platform / Engineering / Data', 'WF-03 CRM-14 Transaction Workflow Foundation', 'High', 'Complete', 'Claim-first receipts (migration 018); compare-and-set deal stage transitions.', 3),
  ('S-020', 'Platform / Engineering / Data', 'WF-04 XML-Driven RE Supermodel (CRM-14E)', 'High', 'Complete', 'XML is the authoritative definition source; RE_supermodel-v1.xml; no Neon deploy yet.', 3),
  ('S-021', 'Platform / Engineering / Data', 'WF-05 Neon Workflow Transaction Adapter', 'High', 'Complete', 'One transaction per engine operation.', 3),
  ('S-022', 'Platform / Engineering / Data', 'WF-06 Workflow Task Completion Seam', 'High', 'Complete', '1:1 engine/canonical task correlation (migration 019); idempotent materialization.', 3),
  ('S-023', 'Platform / Engineering / Data', 'WF-07 Neon Interactive Transaction Handling', 'High', 'Complete', 'WebSocket-pool interactive transactions corrected; proven live on DEV.', 3),
  ('S-024', 'Platform / Engineering / Data', 'WF-08 Workflow Command Receipts & Idempotent Replay', 'High', 'Complete', 'Pending receipt maps to retryable conflict; never a terminal outcome.', 3),
  ('S-025', 'Platform / Engineering / Data', 'WF-09 Workflow Reset & IT Support Diagnostics', 'Medium', 'Complete', 'Read-only anomaly detectors; live terminal-invariant sweep clean.', 3),
  ('S-026', 'Platform / Engineering / Data', 'WF-10 Workflow End-to-End Trust Validation', 'High', 'Complete', 'Duplicate replay, closing-date reschedule, and correlation trust validated.', 3),
  ('S-027', 'Platform / Engineering / Data', 'WF-11 TUNIT Harvest Register', 'Medium', 'Complete', '16 proven mechanisms harvested for the future TUNIT suite.', 3),
  ('S-028', 'Platform / Engineering / Data', 'WF-12 Join Release Concurrency Regression Test', 'Medium', 'Complete', 'Join releases exactly once under simultaneous branch completions.', 3),
  ('S-029', 'Platform / Engineering / Data', 'TUNIT Formal Suite', 'High', 'Planned', 'Recommended next story; converts the harvest register into a runnable suite.', 4),
  ('S-030', 'Platform / Engineering / Data', 'RE Supermodel Deployment to Neon', 'High', 'Planned', 'Deploy command exists; needs a reviewed environment and explicit authorization.', 4),
  ('S-033', 'Platform / Engineering / Data', 'Engine Error / Terminal Semantics (END / ERROR / CONFLICT)', 'Medium-High', 'Open', 'Archaeology §12: typed error/cancelled/conflict outcomes; failed jobs propagate.', 4),
  ('S-034', 'Platform / Engineering / Data', 'Engine Job Lease Requeue & Timer Auto-Advance', 'Medium-High', 'Open', 'Archaeology §8/§10: expired leases requeue; definition-level timers advance tokens.', 4),
  ('S-035', 'Platform / Engineering / Data', 'Engine Join Correlation & Optional Branch Hardening', 'High-ish', 'Open', 'Archaeology §11: optional/cancelled branches; nested fork-join correlation.', 4),
  ('S-036', 'Platform / Engineering / Data', 'Engine Optimistic Concurrency Guard Enforcement', 'High-ish', 'Open', 'Archaeology §7/§8: optimistic guards on move / complete / completeToken.', 4),
  ('S-037', 'Platform / Engineering / Data', 'Application Command Inventory Completion', 'High', 'Open', 'Idempotency and preconditions for every command class (C/D).', 4),
  ('S-038', 'Platform / Engineering / Data', 'Operational Seams: Alerts / Deadlines / SME / Audit', 'Medium-High', 'Open', 'operational-contracts.ts seams; a reviewed deferral of a seam is acceptable.', 4),
  ('S-039', 'Platform / Engineering / Data', 'Domain Event Persistence & Audit Trail', 'Medium', 'Open', 'Decision required; no application_event table exists today.', 4),
  ('S-040', 'Platform / Engineering / Data', 'Media / Attachment & Retention Policy for Provider Ingestion', 'Medium-High', 'Open', 'Policy must precede any byte ingestion; provider URLs are never media.', 4),
  ('S-041', 'Platform / Engineering / Data', 'Workflow Visual Modeler (legacy Story 129 Future Contract)', 'Later', 'Open', 'Future; held by no-UI constraint; round-trips the same XML grammar.', 4)
on conflict (id) do nothing;

commit;

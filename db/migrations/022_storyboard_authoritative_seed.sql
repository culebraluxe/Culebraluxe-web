-- CulebraLuxe Portal
-- Story Board — authoritative master board (8/21) + rollup columns
-- Migration: 022_storyboard_authoritative_seed.sql
--
-- Replaces the inferred S-001…S-041 seed from migration 021 with the
-- human-authored 8/21 master board (74 stories). Adds:
--   completion integer (0–100, human-authored, informational; rollup uses
--                        status scoring, not this value)
--   rollup boolean      (participates in the workstream rollup)
-- Parent stories (CRM-14, CRM-16, PORTAL-01) are stored with rollup = false;
-- their children carry the rollup weight.
--
-- Workstream values are the canonical short codes:
--   PUBLIC, CRM, PORTAL, TXN, ADMIN, AUTH, CONTENT, HARDEN
--
-- Applied to the disposable DEV branch. Applied to PRODUCTION on 2026-08-21
-- (authoritative 74-row board verified; zero S-*; no spec content seeded).

begin;

alter table storyboard_story
    add column if not exists completion integer not null default 0
        check (completion >= 0 and completion <= 100);

alter table storyboard_story
    add column if not exists rollup boolean not null default true;

-- Authoritative reseed: replace all inferred rows with the 8/21 master board.
delete from storyboard_story;

insert into storyboard_story
    (id, workstream, title, priority, status, notes, completion, rollup)
values
  ('CRM-07', 'CRM', 'WhatsApp Intake', 'High', 'Blocked', 'WhatsApp is absent from canonical channel contract. Requires deliberate narrow schema addition. Phone remains canonical identity.', 10, true),
  ('CRM-08', 'CRM', 'Calendar Intake', 'High', 'Partial', 'Provider-neutral adapter exists; calendar channel canonical; deterministic idempotency; no person auto-creation or task noise. Live connector/auth/receipt/cursor remain.', 70, true),
  ('CRM-09A', 'PORTAL', 'Dashboard V2', 'Critical', 'Complete', 'Real Neon situational-awareness dashboard.', 100, true),
  ('CRM-09B', 'PORTAL', 'Needs Review', 'Critical', 'Partial', 'Real unresolved-intake queue exists. Human resolution write actions remain.', 80, true),
  ('CRM-09C', 'PORTAL', 'Relationship Dossier', 'Critical', 'Complete', 'Identity, interests, interactions, tasks, deals and notes/context.', 100, true),
  ('CRM-09D', 'PORTAL', 'Unified Activity Feed', 'High', 'Complete', 'Cross-channel chronological ledger with deterministic links.', 100, true),
  ('CRM-10', 'PORTAL', 'Attention / Follow-Up', 'High', 'Complete', 'Overdue, due-soon, open work and deterministic quiet-client policy.', 100, true),
  ('CRM-11', 'CRM', 'Showings + Offers', 'High', 'Blocked', 'Existing interaction log and offer-price field are insufficient. Requires first-class Showing and Offer domain models.', 15, true),
  ('CRM-12A', 'PORTAL', 'Deals Portfolio V2', 'High', 'Complete', 'Real operational deal portfolio.', 100, true),
  ('CRM-12B', 'PORTAL', 'Deal Workspace', 'High', 'Complete', 'Deal/property/client/tasks/activity/notes workspace.', 100, true),
  ('CRM-13', 'CRM', 'Deal Participants', 'High', 'Partial', 'Current Client/Owner/Seller projection works. Normalized deal_participant model remains future.', 50, true),
  ('CRM-14', 'TXN', 'Transaction Workflow Kernel', 'High', 'Complete', 'Parent story. Generic workflow engine/application seam is operational and exercised. Children CRM-14A through CRM-14I carry rollup weight.', 100, false),
  ('CRM-14A', 'TXN', 'Fork / Required-Branch Semantics', 'High', 'Complete', 'Required parallel branches and deterministic join behavior exercised.', 100, true),
  ('CRM-14B', 'TXN', 'Concurrent Join Contention Proof', 'High', 'Planned', 'Sequential release is proven. Prior Aider test was invalid because it tested invented local state. Real simultaneous contention proof remains.', 20, true),
  ('CRM-14C', 'TXN', 'Closing Date + Reschedule Semantics', 'High', 'Complete', 'Same workflow job correctly survives closing-date movement.', 100, true),
  ('CRM-14D', 'TXN', 'Closing Confirmation + Gates', 'High', 'Complete', 'Closing confirmation and gating exercised through normal application seam.', 100, true),
  ('CRM-14E', 'TXN', 'Closing + Post-Close + Terminal', 'High', 'Complete', 'Closing, post-close recording and terminal completion exercised with invariants green.', 100, true),
  ('CRM-14F', 'TXN', 'Timer / Lease / Requeue Hardening', 'Medium-High', 'Planned', 'Formalize timer expiry, lease/requeue behavior and autonomous advancement semantics.', 20, true),
  ('CRM-14G', 'TXN', 'Workflow Command Inventory Completion', 'Medium-High', 'Planned', 'Finish remaining application commands required by residential transaction model.', 20, true),
  ('CRM-14H', 'TXN', 'Workflow Operational Seams', 'Medium', 'Planned', 'Production operations, observability, recovery and administrative controls.', 10, true),
  ('CRM-14I', 'TXN', 'Domain Event Persistence Decision', 'Medium', 'Planned', 'Decide whether durable domain-event persistence is required beyond current state/audit model.', 0, true),
  ('CRM-15', 'TXN', 'Closing / External SME Orchestration', 'High', 'Partial', 'Required branches, joins, closing date/reschedule, confirmation, close, post-close recording and terminal lifecycle proven. Real SME integrations remain.', 75, true),
  ('CRM-16', 'TXN', 'Documents / Signatures / Transaction Packet', 'Medium-High', 'Planned', 'Parent story. DOC-01 through DOC-05 carry rollup weight.', 0, false),
  ('CRM-17', 'CRM', 'Reporting', 'Medium', 'Complete', 'Operational metrics and distributions without fake forecasting.', 100, true),
  ('CRM-18', 'CRM', 'Contact / Identity Quality', 'Medium', 'Partial', 'Coverage gaps, malformed identities and weak-contact diagnostics exist. Real contact import/ingestion remains.', 85, true),
  ('OPS-01', 'HARDEN', 'System Health', 'High', 'Complete', 'Operational and data-quality health screen.', 100, true),
  ('OPS-02', 'ADMIN', 'Client Administration', 'High', 'Partial', 'Real client operational table exists. Full CRUD remains.', 80, true),
  ('OPS-03', 'ADMIN', 'Property Administration', 'High', 'Partial', 'Real inventory/admin projection exists. Full CRUD remains.', 80, true),
  ('OPS-04', 'ADMIN', 'Media / Document Audit', 'High', 'Complete', 'Hero/gallery/video/document coverage and orphaned-media auditing.', 100, true),
  ('OPS-05', 'ADMIN', 'Deal / Participant Maintenance', 'Medium', 'Planned', 'Write-side maintenance follows participant-model decision.', 10, true),
  ('OPS-06', 'ADMIN', 'Intake / Resolution Administration', 'Medium', 'Partial', 'Needs Review read side exists. Resolution actions remain.', 50, true),
  ('OPS-07', 'ADMIN', 'Persistent Master Story Board', 'High', 'Complete', 'Neon-backed human-maintained Story Board with create/edit/status persistence.', 100, true),
  ('OPS-08', 'ADMIN', 'Story Board Batch / Next Work Selection', 'Medium', 'Planned', 'Add bounded work-selection capability such as Next 20 without building Jira.', 10, true),
  ('PORTAL-01', 'AUTH', 'Portal Entry + Authentication', 'Critical', 'Blocked', 'Parent business story. AUTH-01 through AUTH-05 carry rollup weight.', 15, false),
  ('PORTAL-02', 'PORTAL', 'Navigation IA', 'Medium', 'Complete', 'Work / Operations & Reporting navigation hierarchy shipped.', 100, true),
  ('PORTAL-03', 'PORTAL', 'Lisa Demo Polish', 'High-value polish', 'Complete', 'Dashboard → Attention → Dossier → Deal → Activity reads as one coherent application.', 100, true),
  ('PORTAL-04', 'PORTAL', 'Final Portal Visual Polish', 'Medium', 'Planned', 'Final presentation-level graphics, spacing and visual hierarchy after core data/write surfaces settle.', 0, true),
  ('PX-12', 'PUBLIC', 'View All Photos / Lightbox', 'High', 'Complete', 'Full-screen canonical gallery with touch and keyboard navigation.', 100, true),
  ('PX-13', 'PUBLIC', 'Book Private Viewing', 'High', 'Complete', 'Honest viewing-request flow into CRM intake; not fake calendar booking.', 100, true),
  ('PX-14', 'PUBLIC', 'Similar Properties', 'High', 'Complete', 'Deterministic recommendations using canonical inventory.', 100, true),
  ('PX-15', 'PUBLIC', 'Plans & Site', 'Later', 'Deferred', 'Existing Documents experience is sufficient for now.', 0, true),
  ('PX-17', 'PUBLIC', 'Nearby / Island Context', 'Later', 'Deferred', 'More useful when broader inventory exists.', 0, true),
  ('PX-19', 'PUBLIC', '360 Experience', 'Later', 'Deferred', 'Hardware/content dependent; intentionally deferred.', 0, true),
  ('PX-20', 'PUBLIC', 'Recently Viewed', 'Medium', 'Complete', 'Browser-local, bounded, deduplicated and stale-safe.', 100, true),
  ('PX-21', 'PUBLIC', 'Compare Properties', 'High', 'Planned', 'Strong fit with canonical structured property facts.', 0, true),
  ('PX-22', 'PUBLIC', 'Favorites', 'High', 'Partial', 'Browser-local implementation exists. Identity/CRM-backed persistence remains.', 50, true),
  ('PX-23', 'PUBLIC', 'Saved Searches + Alerts', 'High', 'Planned', 'Search + identity + notification integration.', 0, true),
  ('PX-24', 'PUBLIC', 'Buyers Search / Filter 2.0', 'High', 'Planned', 'Improve query/search contract as inventory expands.', 0, true),
  ('PX-25', 'CONTENT', 'Managed Marketing Content', 'Medium', 'Planned', 'Move appropriate editorial content out of JSX into managed content cleanly.', 0, true),
  ('PX-26', 'CRM', 'Generic Contact → CRM Intake', 'High-ish', 'Blocked', 'Current receipt contract requires property and property-scoped request type. Needs reviewed generic-enquiry contract.', 10, true),
  ('PX-27', 'CONTENT', 'Public Site Final Brand Palette', 'Medium', 'Planned', 'Carry finalized CulebraLuxe luxury/navy visual identity consistently through public site without wholesale redesign.', 0, true),
  ('PX-28', 'CONTENT', 'Public Site Final Responsive / Visual Sweep', 'Medium', 'Planned', 'Final consistency, accessibility and mobile polish after functional work settles.', 0, true),

  ('PLAT-01', 'CONTENT', 'Property Source Consolidation', 'Medium-High', 'Partial', 'Featured homepage is Neon-backed. Remaining Sanity Portfolio/source consolidation and cleanup remain.', 50, true),
  ('ENG-01', 'HARDEN', 'Agent Operating Model', 'High', 'Complete', 'Cline + DeepSeek executes inspect → edit → run → diagnose → fix → retest → commit under bounded stories.', 100, true),
  ('ENG-02', 'HARDEN', 'Shared Read Projections', 'Medium', 'Complete', 'Shared operational counts and property-media coverage extracted narrowly.', 100, true),
  ('ENG-03', 'HARDEN', 'Disposable DEV Database', 'High', 'Complete', 'Dedicated Neon dev branch, DEV environment routing, migration authority and documentation operational.', 100, true),
  ('ENG-04', 'HARDEN', 'TUNIT Formal Regression Suite', 'High', 'Planned', 'Convert expensive one-off workflow proofs into durable setup → execute → assert → teardown tests behind one canonical command.', 15, true),
  ('ENG-05', 'HARDEN', 'TUNIT Harvest Register', 'High', 'Complete', 'Existing workflow proof mechanisms harvested and documented for conversion into durable regression suite.', 100, true),
  ('ENG-06', 'HARDEN', 'Agent DEV Execution Contract', 'High', 'Complete', 'Agent can mutate DEV DB, apply migrations, run app, exercise real data, test, repair and commit.', 100, true),
  ('ENG-07', 'HARDEN', 'Canonical DEV Lifecycle Command', 'Medium', 'Planned', 'Standardize start/restart/status/test interface when useful; avoid infrastructure for infrastructure''s sake.', 0, true),
  ('ENG-08', 'HARDEN', 'Story Execution Evidence', 'Medium', 'Planned', 'Record commit, tests and concise execution result against a human-authored story.', 0, true),
  ('POLISH-01', 'ADMIN', 'Lot Size Display', 'Medium', 'Blocked', 'Casa Luar canonical value/unit pairing must be corrected in canonical data; no UI workaround.', 10, true),
  ('POLISH-02', 'PUBLIC', 'Google Advanced Marker', 'Low', 'Planned', 'Low-priority cleanup when worthwhile.', 0, true),
  ('POLISH-03', 'PUBLIC', 'Gallery Final Polish / Accessibility', 'Low', 'Partial', 'PX-12 V1 complete. Focus restoration, focus trap and image-click polish remain.', 75, true),
  ('AUTH-01', 'AUTH', 'Portal Authentication', 'Critical', 'Planned', 'Establish authenticated Portal entry.', 0, true),
  ('AUTH-02', 'AUTH', 'Portal Authorization', 'Critical', 'Planned', 'Define roles/permissions and protect operational routes and actions.', 0, true),
  ('AUTH-03', 'AUTH', 'Server Action Authorization', 'Critical', 'Planned', 'Ensure all Portal writes enforce identity and authorization server-side.', 0, true),
  ('AUTH-04', 'AUTH', 'Production Secret / Environment Audit', 'High', 'Planned', 'Verify DEV/preview/production separation before broader production write operations.', 0, true),
  ('AUTH-05', 'AUTH', 'Sensitive Administrative Write Audit', 'Medium-High', 'Planned', 'Durable actor/action metadata for high-value administrative mutations.', 0, true),
  ('DOC-01', 'TXN', 'Canonical Transaction Document Model', 'High', 'Planned', 'Define document type, state, ownership and deal association.', 0, true),
  ('DOC-02', 'TXN', 'Transaction Packet', 'High', 'Planned', 'Assemble required transaction documents based on workflow/deal state.', 0, true),
  ('DOC-03', 'HARDEN', 'Signature Provider Seam', 'Medium-High', 'Planned', 'Establish provider-neutral signing boundary before provider implementation.', 0, true),
  ('DOC-04', 'HARDEN', 'BoldSign Integration', 'Medium-High', 'Planned', 'Send/status/completion/webhook integration after provider seam exists.', 0, true),
  ('DOC-05', 'TXN', 'Signed Document Reconciliation', 'Medium', 'Planned', 'Reconcile completed signed artifacts and signature state into canonical transaction model.', 0, true)
on conflict (id) do nothing;

commit;

-- CulebraLuxe Portal
-- CRM-09B — Needs Review human resolution: durable acting-user capture (AUTH-05)
-- Migration: 033_needs_review_resolution_actor.sql
--
-- Adds who/when metadata to the durable intake receipt so the human resolution
-- of a Needs Review item is auditable (AUTH-05 actor metadata). The receipt
-- remains the durable intake record and is never deleted; the read projection
-- (db/needs-review.ts) is unchanged.
--
-- Only the human resolution path (db/needs-review-resolution.ts) writes these
-- columns. Automated pipeline transitions leave them NULL (system actor, no
-- app_user). `resolved_at` records when the human resolution happened;
-- `resolved_by_user_id` records which app_user performed it (NULL until the
-- AUTH-05 runtime session wiring lands).

begin;

alter table website_intake_submission
    add column if not exists resolved_by_user_id uuid
        references app_user(id)
        on delete set null;

alter table website_intake_submission
    add column if not exists resolved_at timestamptz;

create index if not exists idx_website_intake_submission_resolved_by
    on website_intake_submission(resolved_by_user_id)
    where resolved_by_user_id is not null;

commit;

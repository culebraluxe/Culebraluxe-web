-- CulebraLuxe Portal
-- PX-26: reconcile stale Story Board note for PX-26 (no status change)
-- Migration: 043_storyboard_px26_note.sql
--
-- The 8/21 authoritative board (migration 022) describes PX-26 as
-- "Current receipt contract requires property and property-scoped request
-- type. Needs reviewed generic-enquiry contract." — stale: the feature is
-- implemented via contract extension. Migration 011 added the property-less
-- general_enquiry request type to the single website_intake_submission
-- receipt (CHECK: general_enquiry => property_id IS NULL;
-- private_viewing/property_information => property_id IS NOT NULL);
-- lib/website-intake.ts normalizes property-less general enquiries (and
-- rejects one that carries a property id); db/website-intake.ts persists the
-- canonical interaction (event_type 'general_enquiry_submitted') plus the
-- follow-up task with no property_interest row; components/contact.tsx
-- submits requestType='general_enquiry' when there is no property context;
-- /contact and the home page render Contact without propertyContext; the
-- needs-review queue surfaces general enquiries via a property left join
-- (null property context). Identity/idempotency semantics
-- (source_system='website', source_external_id=submissionId) are unchanged;
-- no synthetic property record is ever created. Covered by
-- scripts/verify-website-intake-general-enquiry.mjs (zero Neon access).
--
-- This reconciles the note to the current state. Status ('Blocked') and
-- completion (10) are intentionally preserved — the story remains in
-- progress pending human review.
--
-- Applied to the disposable DEV branch as part of PX-26.

begin;

update storyboard_story
set notes = 'Generic Contact flows through the canonical website intake pipeline via contract extension: migration 011 added the property-less general_enquiry request type to the single website_intake_submission receipt (CHECK: general_enquiry forbids property_id; private_viewing/property_information require it). /contact submits general_enquiry when there is no property context; property-scoped forms are unchanged. The pipeline persists the canonical interaction (event_type general_enquiry_submitted) plus a follow-up task with NO property_interest row and never fabricates a property. Needs-review surfaces general enquiries via a property left join (null property context). Identity/idempotency (source_system=website, source_external_id=submissionId) unchanged. Verified by scripts/verify-website-intake-general-enquiry.mjs (zero Neon access).',
    updated_at = now()
where id = 'PX-26'
  and status = 'Blocked'
  and completion = 10;

commit;

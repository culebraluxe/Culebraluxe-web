-- CulebraLuxe Portal
-- M-2 / PX-26: generic (property-less) website intake
-- Migration: 011_website_intake_general_enquiry.sql
--
-- Architecture note:
-- - Adds a property-less request type `general_enquiry` to website intake.
-- - `property_id` becomes nullable, with an explicit rule:
--     private_viewing / property_information  => property_id IS NOT NULL
--     general_enquiry                          => property_id IS NULL
-- - The existing receipt state machine, retry boundary, and
--   (source_system, source_external_id) idempotency are preserved.
-- - Identity resolution and CRM-04 intake rules are unchanged.

begin;

alter table website_intake_submission
    drop constraint if exists website_intake_submission_request_type_check;

alter table website_intake_submission
    add constraint website_intake_submission_request_type_check
    check (request_type in (
        'private_viewing',
        'property_information',
        'general_enquiry'
    ));

alter table website_intake_submission
    alter column property_id drop not null;

alter table website_intake_submission
    add constraint website_intake_property_rule check (
        (
            request_type in ('private_viewing', 'property_information')
            and property_id is not null
        )
        or
        (
            request_type = 'general_enquiry'
            and property_id is null
        )
    );

commit;

-- CulebraLuxe Portal
-- DISPLAY-NAME SOURCE — provenance for the canonical person display name.
--
-- CORE RULE: IDENTITY IS NOT DISPLAY NAME. A phone number / email in
-- person_identity is never a good client name. This column records where the
-- canonical person.display_name came from so we can:
--   - enrich a phone/email fallback with a trusted Apple Contacts human name
--   - mark identities that cannot be resolved as 'unresolved' (safe fallback)
--   - preserve source provenance and make enrichment idempotent / replay-safe.
--
-- Values are application-managed: 'apple_contacts' (enriched), 'source_evidence'
-- (promoted from a source human name), 'identity_fallback' (phone/email used as
-- a temporary name at promotion), 'unresolved' (no trusted name found).

begin;

alter table person
    add column if not exists display_name_source text;

commit;

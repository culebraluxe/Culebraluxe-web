-- THE PHANTOM REVISION — one wrong ODS revision, and the repair of it.
--
-- Context (docs/agent/MEMORY.md, 2026-09-28 night entry): the Rust port of the Apple Contacts loader
-- canonicalised text with Postgres `btrim`, which strips SPACES ONLY, where the retired TypeScript
-- loader used `String.prototype.trim`. Migration 255 fixed the normalizer (`apple_trim`) and the
-- re-load reported `changed 0 / replay 2855` — correct, because the staged insert dedupes on
-- (source, source_account, source_contact_id, payload_fingerprint) and the corrected fingerprint is
-- the one revision 4 already carries. That dedupe is exactly why the wrong revision was NOT
-- self-healing: revision 5 stayed the latest revision, kept winning `distinct on (…) order by
-- revision desc` in both the projection and the promotion, and has been feeding the warehouse.
--
-- The row (one contact, `E6B1694F-8F5E-40EB-97E0-AB8FDD3734EC:ABPerson`, Juan A. Santa Cruz):
--
--   DEV  f374d68f-bbe9-4a7e-9f1f-9726d4b12a58  revision 5  received 2026-09-28T23:20:03.612Z
--   PROD a455636e-b9cf-4708-864c-2311b6f6ff0f  revision 5  received 2026-09-28T23:20:03.612Z
--
--   stored fingerprint a4168609398f34d969bf7909168ee0a6d606a0471a3141cf1df7935c594b2789
--   (the pre-255 hash of the untrimmed text); the corrected normalizer derives
--   492a91165fc817c9b7c997669a86212b1cd68ac3ecf3b428060cdc9e40781c5c — which is revision 4's stored
--   fingerprint. The two revisions differ by ONE character: revision 5's WORK street is
--   "\nBo. Delicias 17a" where revision 4's is "Bo. Delicias 17a". Nothing else differs.
--   `apple_contacts_fingerprint_audit()` therefore read `checked 4795 / matched 4794 / mismatched 1`.
--
-- Revision 5 cannot be repaired in place: `integration_staged_contact_fingerprint_unique`
-- (source, source_account, source_contact_id, payload_fingerprint) already holds the corrected
-- fingerprint on revision 4, so re-stamping revision 5 collides. Editing its profile alone would
-- leave `payload_fingerprint` disagreeing with its own content — a stored lie about a revision's
-- identity. Deleting the duplicate is what the loader's own rule says: this content is already in the
-- ODS at revision 4, byte for byte, minus a leading newline the bug invented. This is the Captain's
-- "9 repair" (the row was named for his call in the MEMORY entry; authorization given 2026-09-28).
--
-- The delete is guarded twice — by id, and by "still carries a fingerprint that does not re-derive" —
-- so it is a no-op on any environment without the row, and can never delete a healthy revision.
--
-- Two consequences are repaired with it, both of which follow from "revision 4 is the current state":
--
--   1. The landing projection (`l_person` / `l_property`) still holds revision 5's text. It is
--      rebuilt via `apple_contacts_project()`, the designed idempotent path (one transaction, the
--      current snapshot, before/after tallies).
--   2. DEV's canonical `property` row for the WORK address
--      (`apple_contacts:E6B1694F-…:ABPerson:1`, `6d15ac41-ccac-4f60-b500-5b697741f0bb`) already holds
--      `"\nBo. Delicias 17a"`, because DEV was promoted at 2026-09-29T01:23 — after the bad load.
--      PROD's same row was promoted at 2026-09-28T20:39, before it, and is clean. That one row is
--      normalised below; the promotion is NOT re-run by this migration, because re-promoting on PROD
--      would overwrite human fixes made before the manual_override hold (migration 256) existed —
--      the very bug this whole thread is about.
--
-- Two mechanical obstacles, both handled below:
--
--   * `l_person.integration_staged_contact_profile_id` references the staged row ON DELETE RESTRICT, so
--     the landing row must be detached first. It is a DERIVED row (`l_person` is the current-snapshot
--     projection, not canonical) and the projection re-creates it from the revision that survives.
--     Nothing references `l_person`, so removing it costs nothing; `integration_relationship_evidence`
--     and `supersedes_profile_id` are the two other RESTRICT paths and both are empty for this row.
--   * `integration_staged_contact_fingerprint_unique` already holds the corrected fingerprint on
--     revision 4, so re-stamping revision 5 in place would collide.
--
-- Verification after apply, on each target:
--   select apple_contacts_fingerprint_audit();  -- expect matched = checked, mismatched 0
--   select revision, payload_fingerprint from integration_staged_contact_profile
--    where source_contact_id like 'E6B1694F%' order by revision;  -- 4 is the newest, and re-derives
--   select source_revision, payload_fingerprint from l_person
--    where source_contact_id like 'E6B1694F%';  -- 4, and the corrected fingerprint

begin;

do $$
declare
  v_detached integer := 0;
  v_removed integer := 0;
begin
  -- 1. Detach the derived landing row that points at the phantom (RESTRICT), for phantom rows only:
  --    the predicate is the same "does not re-derive" test, so a healthy row can never be touched and
  --    a replay on an environment without the phantom is a no-op.
  delete from l_person lp
   using integration_staged_contact_profile scp
   where lp.integration_staged_contact_profile_id = scp.id
     and scp.id in (
             'f374d68f-bbe9-4a7e-9f1f-9726d4b12a58'::uuid,   -- DEV
             'a455636e-b9cf-4708-864c-2311b6f6ff0f'::uuid)   -- PROD
     and scp.payload_fingerprint
         <> apple_contacts_fingerprint(apple_contacts_raw_from_profile(scp.profile));
  get diagnostics v_detached = row_count;

  -- 2. The phantom revision itself: the card's own state, as revision 4 already holds it.
  delete from integration_staged_contact_profile
   where id in (
           'f374d68f-bbe9-4a7e-9f1f-9726d4b12a58'::uuid,   -- DEV
           'a455636e-b9cf-4708-864c-2311b6f6ff0f'::uuid)   -- PROD
     and payload_fingerprint
         <> apple_contacts_fingerprint(apple_contacts_raw_from_profile(profile));
  get diagnostics v_removed = row_count;

  raise notice '257_apple_phantom_revision_repair: detached % landing row(s), removed % phantom revision(s)',
      v_detached, v_removed;

  if v_removed > 0 then
    -- 3. Revision 4 is the current state now: rebuild l_person / l_property on it.
    perform apple_contacts_project();
  end if;
end
$$;

-- The machine-owned WORK address row the phantom wrote into the canonical warehouse.
update property x
   set address_line1 = nullif(apple_trim(x.address_line1), ''),
       city = nullif(apple_trim(x.city), ''),
       state_or_province = nullif(apple_trim(x.state_or_province), ''),
       postal_code = nullif(apple_trim(x.postal_code), ''),
       country = nullif(apple_trim(x.country), ''),
       location = nullif(apple_trim(x.location), ''),
       updated_at = now()
 where x.source_type = 'apple_contacts'
   and x.source_listing_key like 'apple_contacts:E6B1694F-8F5E-40EB-97E0-AB8FDD3734EC:%'
   and (x.address_line1 is distinct from nullif(apple_trim(x.address_line1), '')
     or x.city is distinct from nullif(apple_trim(x.city), '')
     or x.state_or_province is distinct from nullif(apple_trim(x.state_or_province), '')
     or x.postal_code is distinct from nullif(apple_trim(x.postal_code), '')
     or x.country is distinct from nullif(apple_trim(x.country), '')
     or x.location is distinct from nullif(apple_trim(x.location), ''));

commit;

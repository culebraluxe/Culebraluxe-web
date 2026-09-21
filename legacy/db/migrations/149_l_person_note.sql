-- CulebraLuxe
-- Capture the Apple Contacts NOTES field (the operator's own context).
-- Migration: 149_l_person_note.sql
--
-- WHY (2026-09-10, captain): the Contacts Notes field is where the operator records
-- context that must not be lost — "house is purple", "wants waterfront", "brother of
-- Jack". It is the reason the name field no longer gets used as a memory aid
-- ("Puerple House" in the name field beat the real name for weeks). Notes are CONTEXT,
-- never a name and never an identity: this column is projected only from
-- profile->>'note' and is never read as a display name or an identity key.
--
-- The exporter previously did not even request CNContactNoteKey, so nothing was read.
-- Non-destructive and additive: nullable text, no backfill, no rewrite.
-- Existing rows keep NULL until the next contacts load.

begin;

alter table l_person
    add column if not exists note text;

comment on column l_person.note is
    'Free-text Notes from the Apple Contacts source (operator context). Never a name, never an identity key.';

commit;

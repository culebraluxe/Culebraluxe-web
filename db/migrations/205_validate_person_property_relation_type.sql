-- 205_validate_person_property_relation_type.sql
--
-- Completes migration 203 in its own transaction.
--
-- 203 re-created `person_property_relation_type_check` with the two owner/buyer roles the Regrid feed needs
-- (`previous_owner`, `buyer`) and added it NOT VALID, because the migration runner sends each file as ONE
-- statement inside a transaction and the lint refuses both alternatives: a plain ADD CONSTRAINT blocks writes
-- during its scan; NOT VALID + VALIDATE in the same transaction blocks reads during the validation.
--
-- This file is what makes the constraint REAL. Until it runs, `person_property` accepts any relation_type that
-- the CHECK does not see, because an unvalidated constraint is not applied to existing rows.
--
-- VALIDATE takes a SHARE UPDATE EXCLUSIVE lock: reads and writes both continue, only a competing ALTER or VACUUM
-- would wait. On a table of this size the scan is instant; the shape is what a much larger `person_property` will
-- need, so it is written the correct way now rather than when it hurts.

set lock_timeout = '5s';
set statement_timeout = '30s';

begin;

alter table person_property validate constraint person_property_relation_type_check;

commit;

-- Proof rather than assertion: the constraint is validated, not merely present.
do $$
declare
    validated boolean;
begin
    select convalidated into validated
      from pg_constraint
     where conname = 'person_property_relation_type_check';

    if validated is not true then
        raise exception 'person_property_relation_type_check is still NOT VALID';
    end if;
end $$;

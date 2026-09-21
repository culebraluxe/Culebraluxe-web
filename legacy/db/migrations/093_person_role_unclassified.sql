-- 093_person_role_unclassified.sql
-- Canonical Person roles gain 'unclassified' for passively promoted (Apple)
-- people whose buyer/seller status is not yet known.
--
-- Existing buyer/seller/both rows are NOT rewritten: we lack provenance to
-- safely assume every Apple-linked buyer was incorrectly classified, so those
-- legitimate manual/business classifications are preserved.

begin;

-- Drop whichever check constraint currently constrains person.role (the
-- original is an inline, auto-named constraint). Matching on the definition
-- keeps the migration robust regardless of the auto-generated name.
do $$
declare
  cname text;
begin
  select conname into cname
    from pg_constraint
    where conrelid = 'person'::regclass
      and contype = 'c'
      and pg_get_constraintdef(oid) like '%''buyer''%'
    limit 1;
  if cname is not null then
    execute format('alter table person drop constraint %I', cname);
  end if;
end $$;

alter table person
  add constraint person_role_check
    check (role in ('buyer', 'seller', 'both', 'unclassified'));

commit;

-- Bounded PRODUCTION repair for the Alicia Geigel identity graph discovered on
-- 2026-09-02. This is intentionally Person-ID and identity asserted; it is not
-- a name-based mastering rule.
--
-- Survivor: canonical buyer Person with alicia.geigel@gmail.com
-- Losers:   phone-only Apple Person and Yahoo-only Apple Person
--
-- Execute all statements in one transaction. Refresh the three Client MVs only
-- after the transaction commits successfully.

-- TRANSACTION_STATEMENT
create temporary table person_consolidation_map (
  loser uuid primary key,
  winner uuid not null
) on commit drop;

-- TRANSACTION_STATEMENT
insert into person_consolidation_map (loser, winner) values
  ('968d4b17-e11c-4b42-8e0d-e64fdb67b967', '21a49837-a271-461d-aaa0-12d84cad529a'),
  ('525f8bde-ab98-4a72-a0d1-d32a1ef66947', '21a49837-a271-461d-aaa0-12d84cad529a');

-- TRANSACTION_STATEMENT
do $assert$
declare
  survivor_count integer;
  loser_count integer;
  expected_identity_count integer;
begin
  select count(*) into survivor_count
  from person p
  where p.id = '21a49837-a271-461d-aaa0-12d84cad529a'
    and p.archived_at is null
    and p.display_name = 'Alicia  Geigel'
    and exists (
      select 1 from person_identity pi
      where pi.person_id = p.id
        and pi.identity_type = 'email'
        and lower(pi.identity_value) = 'alicia.geigel@gmail.com'
    );

  select count(*) into loser_count
  from person p
  where p.id in (
      '968d4b17-e11c-4b42-8e0d-e64fdb67b967',
      '525f8bde-ab98-4a72-a0d1-d32a1ef66947'
    )
    and p.archived_at is null;

  select count(*) into expected_identity_count
  from person_identity pi
  where (pi.person_id = '968d4b17-e11c-4b42-8e0d-e64fdb67b967'
         and pi.identity_type = 'phone'
         and regexp_replace(pi.identity_value, '[^0-9]', '', 'g') = '17274201806')
     or (pi.person_id = '525f8bde-ab98-4a72-a0d1-d32a1ef66947'
         and pi.identity_type = 'email'
         and lower(pi.identity_value) = 'alicia.geigel@yahoo.com');

  if survivor_count <> 1 or loser_count <> 2 or expected_identity_count <> 2 then
    raise exception 'Alicia repair precondition changed (survivor %, losers %, asserted identities %)',
      survivor_count, loser_count, expected_identity_count;
  end if;

  if exists (
    select 1 from person_identity
    where person_id in (
      '968d4b17-e11c-4b42-8e0d-e64fdb67b967',
      '525f8bde-ab98-4a72-a0d1-d32a1ef66947'
    )
    and not (
      (identity_type = 'phone' and regexp_replace(identity_value, '[^0-9]', '', 'g') = '17274201806')
      or (identity_type = 'email' and lower(identity_value) = 'alicia.geigel@yahoo.com')
    )
  ) then
    raise exception 'Alicia repair aborted: a loser gained an unasserted identity';
  end if;
end
$assert$;

-- TRANSACTION_STATEMENT
select id from person
where id in (
  '21a49837-a271-461d-aaa0-12d84cad529a',
  '968d4b17-e11c-4b42-8e0d-e64fdb67b967',
  '525f8bde-ab98-4a72-a0d1-d32a1ef66947'
) for update;

-- TRANSACTION_STATEMENT
update person_identity pi
set person_id = m.winner, updated_at = now()
from person_consolidation_map m
where pi.person_id = m.loser;

-- TRANSACTION_STATEMENT
do $merge$
declare
  fk record;
  m record;
  remains boolean;
begin
  for fk in
    select c.conrelid::regclass::text as table_name, a.attname as column_name
    from pg_constraint c
    join lateral unnest(c.conkey) with ordinality as k(attnum, ord) on true
    join pg_attribute a on a.attrelid = c.conrelid and a.attnum = k.attnum
    where c.contype = 'f'
      and c.confrelid = 'person'::regclass
      and c.conrelid <> 'person_identity'::regclass
      and cardinality(c.conkey) = 1
  loop
    for m in select loser, winner from person_consolidation_map loop
      begin
        execute format('update %s set %I = $1 where %I = $2', fk.table_name, fk.column_name, fk.column_name)
          using m.winner, m.loser;
      exception when unique_violation then
        raise exception using
          errcode = '23505',
          message = format(
            'Alicia consolidation blocked at %s.%s (loser=%s winner=%s)',
            fk.table_name, fk.column_name, m.loser, m.winner
          );
      end;
    end loop;
  end loop;

  for fk in
    select c.conrelid::regclass::text as table_name, a.attname as column_name
    from pg_constraint c
    join lateral unnest(c.conkey) with ordinality as k(attnum, ord) on true
    join pg_attribute a on a.attrelid = c.conrelid and a.attnum = k.attnum
    where c.contype = 'f'
      and c.confrelid = 'person'::regclass
      and c.conrelid <> 'person_identity'::regclass
      and cardinality(c.conkey) = 1
  loop
    for m in select loser, winner from person_consolidation_map loop
      execute format('select exists(select 1 from %s where %I = $1)', fk.table_name, fk.column_name)
        into remains using m.loser;
      if remains then
        raise exception 'Alicia consolidation invariant failed: % still references loser %',
          fk.table_name, m.loser;
      end if;
    end loop;
  end loop;
end
$merge$;

-- The phone handle was reconciled before semantic NANP matching was fixed.
-- Establish it explicitly now that the asserted phone identity belongs to the
-- surviving Person. The next Mac Apple sync will idempotently materialize its
-- 536 detailed messages.
-- TRANSACTION_STATEMENT
insert into integration_source_person_link (
  source, source_account, source_identity_key, canonical_person_id,
  link_method, link_reason
) values (
  'apple_messages', 'E:lisapenfield@icloud.com', '+17274201806',
  '21a49837-a271-461d-aaa0-12d84cad529a',
  'operator_confirmed', 'bounded_alicia_identity_consolidation_2026_09_02'
)
on conflict (source, source_account, source_identity_key) do update
set canonical_person_id = excluded.canonical_person_id,
    link_method = excluded.link_method,
    link_reason = excluded.link_reason,
    updated_at = now();

-- TRANSACTION_STATEMENT
update integration_relationship_evidence
set canonical_person_id = '21a49837-a271-461d-aaa0-12d84cad529a',
    review_state = 'exact_linked',
    match_method = 'source_link',
    match_confidence = 'exact',
    match_reason = 'bounded_alicia_identity_consolidation_2026_09_02',
    updated_at = now()
where source = 'apple_messages'
  and source_account = 'E:lisapenfield@icloud.com'
  and source_identity_key = '+17274201806'
  and canonical_person_id is null
  and review_state = 'unmatched';

-- TRANSACTION_STATEMENT
update person p
set archived_at = now(), updated_at = now()
from person_consolidation_map m
where p.id = m.loser;

-- TRANSACTION_STATEMENT
do $verify$
begin
  if (select count(*) from person where id in (
      '21a49837-a271-461d-aaa0-12d84cad529a',
      '968d4b17-e11c-4b42-8e0d-e64fdb67b967',
      '525f8bde-ab98-4a72-a0d1-d32a1ef66947'
    ) and archived_at is null) <> 1 then
    raise exception 'Alicia consolidation invariant failed: expected one active Person';
  end if;

  if (select count(*) from person_identity
      where person_id = '21a49837-a271-461d-aaa0-12d84cad529a') <> 3 then
    raise exception 'Alicia consolidation invariant failed: expected three identities';
  end if;

  if exists (select 1 from integration_relationship_evidence
             where source = 'apple_messages'
               and source_account = 'E:lisapenfield@icloud.com'
               and source_identity_key = '+17274201806'
               and (canonical_person_id <> '21a49837-a271-461d-aaa0-12d84cad529a'
                    or review_state <> 'exact_linked')) then
    raise exception 'Alicia phone iMessage evidence was not linked to survivor';
  end if;
end
$verify$;

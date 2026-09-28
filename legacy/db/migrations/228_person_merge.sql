-- MERGE TWO RECORDS OF ONE PERSON. Apple split people into several records — one with the phone, one with the email,
-- one empty — and the same person entered twice. merge_person(golden, duplicate) folds the duplicate into the golden
-- record, which is what the Records screen's "Merge into this person" and the duplicate cleanup both call:
--
--   1. every field the golden record lacks (null, or empty text) is filled from the duplicate — the golden record's
--      own values always win;
--   2. everything that points at the duplicate is moved to the golden record: every foreign key to person, found from
--      the catalog (so a table added later is included), and the three references kept as text (project,
--      wbs_item entity, workflow trace). A row the golden record already has an equivalent of (a unique or check
--      rule refuses the move — both were linked to the same property, say) is dropped instead;
--   3. a person linked to themself by the merge is unlinked, and the duplicate is deleted.
--
-- It runs inside the caller's transaction: all of it happens, or none. It answers {"moved": n, "dropped": n}.

create or replace function merge_person(p_golden uuid, p_duplicate uuid) returns jsonb
language plpgsql as $$
declare
    link record;
    row record;
    assignments text;
    moved integer := 0;
    dropped integer := 0;
begin
    if p_golden = p_duplicate then
        raise exception 'merge_person: a person cannot be merged into themself';
    end if;
    perform 1 from person where id in (p_golden, p_duplicate) for update;
    if (select count(*) from person where id in (p_golden, p_duplicate)) <> 2 then
        raise exception 'merge_person: both people must exist';
    end if;

    -- 1. Fill what the golden record lacks.
    select string_agg(
               case when data_type in ('text', 'character varying')
                    then format('%1$I = coalesce(nullif(g.%1$I, %2$L), d.%1$I)', column_name, '')
                    else format('%1$I = coalesce(g.%1$I, d.%1$I)', column_name)
               end, ', ' order by ordinal_position)
      into assignments
      from information_schema.columns
     where table_schema = 'public' and table_name = 'person' and is_generated = 'NEVER'
       and column_name not in ('id', 'created_at', 'updated_at');
    execute format('update person g set %s, updated_at = now() from person d where g.id = %L and d.id = %L',
                   assignments, p_golden, p_duplicate);

    -- 2. Move everything that points at the duplicate.
    for link in
        select c.conrelid::regclass as tbl, a.attname as col
          from pg_constraint c
          join pg_attribute a on a.attrelid = c.conrelid and a.attnum = any (c.conkey)
         where c.contype = 'f' and c.confrelid = 'person'::regclass
    loop
        for row in execute format('select ctid from %s where %I = %L', link.tbl, link.col, p_duplicate) loop
            begin
                execute format('update %s set %I = %L where ctid = %L', link.tbl, link.col, p_golden, row.ctid);
                moved := moved + 1;
            exception when unique_violation or check_violation then
                execute format('delete from %s where ctid = %L', link.tbl, row.ctid);
                dropped := dropped + 1;
            end;
        end loop;
    end loop;
    update project set person_id = p_golden::text where person_id = p_duplicate::text;
    update wbs_item set entity_id = p_golden::text where entity_type = 'person' and entity_id = p_duplicate::text;
    update workflow_execution_trace_event set person_id = p_golden::text where person_id = p_duplicate::text;

    -- 3. No one is their own relation; the duplicate goes.
    delete from person_person where person_id = related_person_id;
    delete from person where id = p_duplicate;

    return jsonb_build_object('moved', moved, 'dropped', dropped);
end
$$;

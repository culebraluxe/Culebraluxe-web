-- Keep observations from disabled filing rules without pinning the source scan cursor.

begin;

alter table forge_learn_scan_state
    add column if not exists deferred_observations jsonb not null default '[]'::jsonb;

do $$
begin
    if not exists (
        select 1 from pg_constraint
         where conname = 'forge_learn_deferred_observations_array'
           and conrelid = 'forge_learn_scan_state'::regclass
    ) then
        alter table forge_learn_scan_state
            add constraint forge_learn_deferred_observations_array
            check (jsonb_typeof(deferred_observations) = 'array');
    end if;
end
$$;

comment on column forge_learn_scan_state.deferred_observations is
    'Durable learning observations whose filing rule is disabled; provenance remains on each observation and does not block source cursor progress.';

commit;

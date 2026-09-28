-- A WBS deadline is separate from the planned work span.
-- Existing items remain unscheduled; no legacy three-day bars are backfilled.
alter table wbs_item
    add column if not exists planned_start date,
    add column if not exists planned_finish date;

do $$ begin
    if not exists (
        select 1 from pg_constraint where conname = 'wbs_item_planned_dates_order'
    ) then
        alter table wbs_item add constraint wbs_item_planned_dates_order
            check (planned_start is null or planned_finish is null or planned_start <= planned_finish);
    end if;
end $$;

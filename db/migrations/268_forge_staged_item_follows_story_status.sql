-- 268_forge_staged_item_follows_story_status.sql
--
-- WHY: `forge doctor` reported `board vs table: DRIFT - this is a bug, report it (board 0 vs table 1)` against
-- production on 2026-10-04. Nothing was `Batched` on the board, yet the staging batch `9f80316c` still counted one
-- member: `LEARN-SWALLOWED-CATCH-E2E-PORTAL-NAV-SMOKE-MJS-1790666636`, a `learn` item staged by the learn pass
-- (`ForgeControlDao::stage_learn_item`, db/src/forge_control.rs:339), whose story `forge reset` had since returned to
-- `Planned` (db/src/forge_reset.rs:111, "Return the story to Planned") without withdrawing the batch row.
--
-- THE FACT, AND WHICH HALF WAS WRONG. "This story is staged in the next flight" is written twice:
-- `storyboard_story.status = 'Batched'` -- the board's half, read by `forge_read.story_statuses` -- and one
-- `forge_batch_item` row with `state = 'Staged'` -- the batch table's half, counted by `tech.batch_select()`'s
-- `(select count(*) from forge_batch_item i where i.batch_id=b.id) as story_count`. Membership is DERIVED from the
-- status: a story is in the next flight because the board says `Batched`, never because a row survived. So the status
-- is the one fact and the item must follow it, and the reset path was the writer that forgot -- while four others
-- (`stage_learn_item`, `tech.stage_story_for_batch` + `tech.story_status`, `tech.unstage_story_from_batch`,
-- `tech.launch_flight`) each carry their own half by hand. The next writer to forget would drift the same way, which
-- is why the rule goes on the column rather than into a sixth caller.
--
-- WHERE THE RULE LIVES NOW: on the status, in the database, so every writer obeys it -- the reset path, the Cockpit's
-- move-to-bench, a hand-run `update storyboard_story set status=...`, and any DAO added later. A story that LEAVES
-- `Batched` withdraws its staged membership.
--
-- DEFERRED, AND WHY THAT IS NOT A DETAIL. A flight being fired writes a story's status before it stamps that story's
-- item: `tech.launch_flight` runs `LAUNCH_FLIGHT_FIRE_STORIES_SQL` (members -> `Ready`, selected from the very items
-- that are still `Staged`) and only then `LAUNCH_FLIGHT_QUEUE_ITEMS_SQL` (those items -> `Queued`). An immediate
-- trigger would read that interval as a departure and delete the flight it is launching. The invariant is about
-- COMMITTED states, not intermediate ones, so this is a constraint trigger deferred to commit: a multi-statement
-- writer is judged once, on its final state. `launch_flight` commits those statements together (db/src/tech.rs) for
-- exactly this reason, and `run_text_dev.rs` plans the same three statements against the live schema.
--
-- THE DIRECTION NOT ENFORCED HERE, DELIBERATELY. The mirror rule ("a `Staged` item needs a `Batched` story") is not
-- added, because the Cockpit stages in two transactions -- `stage_story_for_batch` inserts the item (db/src/tech.rs:389)
-- and its caller writes the status -- so an item-side constraint would judge the intent in progress and refuse or
-- delete it. Entry is consistent in every writer today and a stray item is what the doctor reports; making entry
-- atomic (status first, both in one transaction) is its own story, not a rider on this one.
--
-- WHAT IT DOES NOT TOUCH: history. A `Fired` or `Cancelled` batch keeps every row it has, whatever state its items are
-- in -- that is the record of what flew, and the batch screen reads its `Skipped`/`Queued`/`Failed` members back from
-- it. Only `Staged` items -- "about to fly" -- are withdrawn, and only from a batch that has not fired.

begin;

-- A story that is no longer `Batched` holds no staged membership in a flight that has not fired.
--
-- The CURRENT status decides, not this event's `new.status`: a deferred trigger fires once per update event and one
-- transaction may have moved a status more than once, so only the committed truth can answer "is this story staged".
create or replace function forge_withdraw_staged_items()
returns trigger
language plpgsql
as $$
begin
    if exists (
        select 1 from storyboard_story s where s.id = new.id and s.status <> 'Batched'
    ) then
        delete from forge_batch_item i
         using forge_batch b
         where i.batch_id = b.id
           and i.story_id = new.id
           and i.state = 'Staged'
           and b.status in ('Staged', 'Scheduled');
    end if;
    return null;
end
$$;

comment on function forge_withdraw_staged_items() is
    'Withdraws a story''s staged batch membership once the story is no longer `Batched`: the status is the one fact and the item follows it. Fired and cancelled batches are history and are never touched (migration 268).';

drop trigger if exists storyboard_story_withdraw_staged_items on storyboard_story;
create constraint trigger storyboard_story_withdraw_staged_items
    after update of status on storyboard_story
    deferrable initially deferred
    for each row
    when (old.status = 'Batched' and new.status is distinct from 'Batched')
    execute function forge_withdraw_staged_items();

-- REPAIR, LABELLED AS REPAIR: the rows that already contradict the status column, removed by exactly the rule above.
-- In production on 2026-10-04 that is one row -- the `learn` item named in the WHY -- and the story it claimed stays
-- `Planned` on the board for a Lead to pick up; what goes is only the claim that it was staged for the next flight.
delete from forge_batch_item i
 using forge_batch b, storyboard_story s
 where i.batch_id = b.id
   and s.id = i.story_id
   and i.state = 'Staged'
   and b.status in ('Staged', 'Scheduled')
   and s.status <> 'Batched';

commit;

-- 258_reopen_stranded_ready_work_items.sql
--
-- WHY: `forge clean` (rust/core/db/src/forge_reset.rs:164-174) cancels open `agent_work_item` rows without
-- moving `storyboard_story.status` off 'Ready'. The dispatch trigger that turns a Ready story into a work
-- item fires only on INSERT or on a *change* of status (db/migrations/025_agent_work_queue.sql:104), so a
-- story left at Ready with a Cancelled item is never re-queued: it stays on the board as Ready and can
-- never be dispatched again. Reproduced 2026-09-29: `pnpm forge:clean` reported
-- `cancelled stale open work items: 8` and left `open work items: 0` with 8 stories still Ready, and the
-- next worker tick ended `idle: no work`.
--
-- WHAT: re-open the newest Cancelled item of every story that is currently Ready and has no open item.
-- `queued_at` is kept, so the FIFO order `next_ready_work` uses (rust/core/db/src/forge_control.rs:264)
-- is the order the story had before the sweep. Idempotent: a story that already has an open item is
-- untouched, and a re-run after the repair matches no rows.
--
-- SCOPE: this repairs the rows, not the writer. A later `clean` strands the same way; taking
-- `storyboard_story.status` off Ready (what `reset` does, forge_reset.rs:108-116) or re-queueing after the
-- cancel is a story of its own, not this migration.
--
-- The claim fields are cleared and `error_text` is written in the vocabulary the engine's own recovery
-- uses (forge_reset.rs:326-331), so a re-opened item is indistinguishable from one the engine released
-- itself. `storyboard_story_run_id` is left alone: it names the story's last run, which is evidence.

with stranded as (
    select distinct on (i.story_id) i.id
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.state = 'Cancelled'
       and s.status = 'Ready'
       and not exists (
             select 1 from agent_work_item o
              where o.story_id = i.story_id
                and o.state in ('Ready', 'Claimed', 'Running', 'Paused'))
     order by i.story_id, coalesce(i.finished_at, i.updated_at) desc nulls last
)
update agent_work_item a
   set state       = 'Ready',
       claimed_by  = null,
       claimed_at  = null,
       started_at  = null,
       finished_at = null,
       error_text  = 're-opened by 258: a clean stranded this Ready story; awaiting a fresh attempt',
       updated_at  = now()
  from stranded
 where a.id = stranded.id;

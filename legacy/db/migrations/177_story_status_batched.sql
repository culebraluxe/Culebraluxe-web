-- 177_story_status_batched.sql
--
-- ENGINE BATCH: a staging bucket for stories chosen for the next group handed to the engine.
--
-- The captain's ask: drag a story from OPEN (or the bench) into ENGINE BATCH, then send the whole
-- batch to the engine on demand. It cannot reuse an existing status:
--   - 'Ready' IS the dispatch trigger (`agent_work_item_dispatch()` fires on a status change to
--     Ready and inserts a work item), so a staging bucket that wrote Ready would dispatch the moment
--     you staged something - the opposite of staging;
--   - 'Planned' is BACKLOG, 'In Progress' is OPEN, 'Deferred' is NEXT VERSION. Reusing any of them
--     would put a batched story in two columns at once.
-- So the status vocabulary gains exactly one value. It is a STATUS, not a bucket-only concept,
-- because the persistence is the status column and the board reads status -> lifecycle.
--
-- Non-destructive: only the CHECK constraint is replaced; no row is touched, no value removed.

alter table storyboard_story
  drop constraint if exists storyboard_story_status_check;

alter table storyboard_story
  add constraint storyboard_story_status_check
  check (status = any (array[
    'Planned'::text,
    'Ready'::text,
    'In Progress'::text,
    'Complete'::text,
    'Partial'::text,
    'Blocked'::text,
    'Failed'::text,
    'Deferred'::text,
    'Hold'::text,
    'Batched'::text
  ]));

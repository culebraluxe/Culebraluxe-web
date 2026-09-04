-- Forge industrial recovery: a worker process/host interruption is neither a
-- successful implementation result nor a story failure. Preserve it as a
-- distinct run outcome so the same work item/worktree can be retried without
-- falsifying acceptance evidence.

begin;

alter table storyboard_story_run
    drop constraint if exists storyboard_story_run_result_status_check;

alter table storyboard_story_run
    add constraint storyboard_story_run_result_status_check
        check (result_status is null or result_status in
            ('Complete', 'Partial', 'Blocked', 'Failed', 'Deferred', 'Hold',
             'Cancelled', 'Interrupted'));

commit;

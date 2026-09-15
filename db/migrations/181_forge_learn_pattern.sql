-- 181_forge_learn_pattern.sql
--
-- PHASE 3 OF THE FACTORY (ENG-FORGE-FACTORY-01, Object 3): traces write work.
--
-- WHAT THIS COLUMN IS FOR: the learn loop may file at most ONE item per unattended pass, and it must not
-- file a second item for a pattern that already has one open. That rule needs the pattern key stored where
-- the open work is, and it is enforced here rather than in code, because the failure mode is a loop that
-- quietly files the same silent-failure finding every three minutes all night.
--
-- THE UNIQUE INDEX IS THE GATE. A partial index over the OPEN states means the database itself refuses a
-- second open item for one pattern key - not "the code checks first", which is a race between two worker
-- passes and a lie the moment the check and the insert are in different transactions.
--
-- WHY THE KEY IS ALSO ON forge_batch_item: a staged member is open work too. Dedupe that only looked at
-- agent_work_item would happily stage a learn item that is already sitting in the batch waiting to fire.
--
-- Non-destructive: two nullable columns and one partial index. Existing rows keep NULL, which is the
-- honest value: they were not filed by the learn loop.
--
-- NOT IN THIS MIGRATION, deliberately: a decision is never auto-promoted and work is never auto-merged.
-- Those are policy, and policy lives in code where it can be read and tested.

alter table agent_work_item
    add column if not exists learn_pattern_key text null;

alter table forge_batch_item
    add column if not exists learn_pattern_key text null;

-- One OPEN item per pattern. `learn_pattern_key is null` rows are excluded, so this index constrains only
-- the learn loop's own work and cannot affect a normal dispatch.
create unique index if not exists agent_work_item_learn_pattern_open_idx
    on agent_work_item (learn_pattern_key)
    where learn_pattern_key is not null
      and state in ('Ready', 'Claimed', 'Running', 'Paused');

-- The dedupe query also asks "is this pattern already staged?", which is the other half of "open".
create index if not exists forge_batch_item_learn_pattern_idx
    on forge_batch_item (learn_pattern_key)
    where learn_pattern_key is not null;

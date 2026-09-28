-- 179_forge_kind_policy.sql
--
-- PHASE 1 OF THE FACTORY (ENG-FORGE-FACTORY-01, Grok's packet, 2026-09-15): every piece of work
-- carries a KIND and a MODEL POLICY before a lane speaks, so a night batch can be cheap on purpose
-- instead of token-maxxing by accident.
--
-- WHY TWO COLUMNS AND NOT A NEW TABLE: the packet's Object 1 is a router, not a registry. Six kinds and
-- two policies, chosen before the work is queued, stored where the work already is. `forge_batch` is the
-- job stream (migration 178) and `agent_work_item` is the queue, so both get the answer copied at the
-- moment it is known.
--
-- WHY THE COPY IS CODE, NOT THIS MIGRATION: `agent_work_item` rows are created by the
-- `agent_work_item_dispatch()` TRIGGER on `storyboard_story` (migration 025, last replaced in 146),
-- which sees only the story row - it cannot know a batch item's kind. The fire path
-- (`fireForgeBatch`, `db/forge-batch.ts`) is the only code that knows both the batch item and the
-- story, so it writes the copy immediately after the dispatch it just performed, in the same
-- transaction. Coupling the trigger to `forge_batch_item` was the alternative and is worse: it would
-- make dispatching a story that was never batched depend on batch tables.
--
-- DEFAULTS ARE THE POLICY, on purpose:
--   * `model_policy` defaults to `cheap` on the batch row, so the unattended night run is cheap
--     WITHOUT anybody remembering to say so - that is the whole point of the packet.
--   * `kind` defaults to `fix`, the honest answer for "we know it is broken", which is most work.
--
-- BACKWARD COMPATIBLE for existing rows: `not null default` backfills 178-era batches and any queue
-- rows, so nothing reads null. `agent_work_item` allows null (an item queued before this migration has
-- no kind, and pretending otherwise would be a lie in the data).
--
-- Non-destructive: three added columns, two check constraints, no existing row or column touched.

alter table forge_batch
    add column if not exists model_policy text not null default 'cheap';

alter table forge_batch
    drop constraint if exists forge_batch_model_policy_check,
    add constraint forge_batch_model_policy_check
        check (model_policy in ('cheap', 'judgment'));

alter table forge_batch_item
    add column if not exists kind text not null default 'fix';

alter table forge_batch_item
    drop constraint if exists forge_batch_item_kind_check,
    add constraint forge_batch_item_kind_check
        check (kind in ('qa', 'fix', 'feature', 'crm', 'judgment', 'learn'));

alter table agent_work_item
    add column if not exists kind text null,
    add column if not exists model_policy text null;

alter table agent_work_item
    drop constraint if exists agent_work_item_kind_check,
    add constraint agent_work_item_kind_check
        check (kind is null or kind in ('qa', 'fix', 'feature', 'crm', 'judgment', 'learn')),
    drop constraint if exists agent_work_item_model_policy_check,
    add constraint agent_work_item_model_policy_check
        check (model_policy is null or model_policy in ('cheap', 'judgment'));

-- "What is queued cheaply right now?" and the ROI strip's first question, by kind.
create index if not exists agent_work_item_routing_idx on agent_work_item (kind, model_policy)
    where state in ('Ready', 'Claimed', 'Running');

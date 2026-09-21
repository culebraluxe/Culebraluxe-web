-- CulebraLuxe Portal
-- FORGE role contract — the decision in FIELDS, not in JSON.
-- Migration: 170_forge_role_contract.sql
--
-- WHY THIS EXISTS (the thing that cost an evening): every role contract used to be
-- JSON embedded in the model's chat reply — `LEAD_ROUTING: {...}`, `SMITH_CANDIDATE:
-- {...}`, `FORGE_ARCHITECT_HANDOFF: {...}`. A real model produces the OBJECT reliably
-- and the LABEL unreliably: it emitted a perfect routing proposal with no
-- `LEAD_ROUTING:` prefix and the run held, discarding a decision the engine already
-- had in hand. We then spent the evening making parsers more forgiving, which treats
-- the symptom: the failure is the CHANNEL, not the parser.
--
-- So the decision moves into columns. The model runs one command with flags
-- (scripts/forge-handoff.mjs); the DB enforces the contract at the moment of writing:
-- a bad value fails the WRITE with a database error the model can read and fix,
-- instead of silently becoming unparseable text that holds a run 18 minutes later.
--
-- No JSON, no marker, no envelope, no prose scanning. A NULL column is a missing
-- decision and the gate holds on it deterministically.
--
-- The JSON parsers stay for now as a FALLBACK (older runs, and any harness that has
-- not been switched over). The columns win when they are present.

begin;

create table if not exists forge_role_contract (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    process_instance_id text not null,
    task_id text not null,
    node_id text not null,
    attempt integer not null default 1,

    -- The decision, in fields. Nulls are allowed while a row is being filled in;
    -- the GATE is what holds on an incomplete contract, not the column.
    decision text null,
    size text null,
    size_reason text null,
    reason text null,
    assignment_count integer null,
    finding_ids text[] not null default '{}',
    merge_checks text[] not null default '{}',
    surface_scope text[] not null default '{}',

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint forge_role_contract_decision_check
        check (decision is null or decision in ('SOLO', 'SMITH', 'SPLIT', 'HOLD')),
    constraint forge_role_contract_size_check
        check (size is null or size in ('SMALL', 'MEDIUM', 'LARGE')),
    constraint forge_role_contract_assignment_count_check
        check (assignment_count is null or assignment_count between 0 and 8),
    constraint forge_role_contract_attempt_check
        check (attempt >= 1)
);

-- One contract per (task, node, attempt): a retry writes a NEW attempt row rather
-- than mutating the record of the decision that was actually made.
create unique index if not exists forge_role_contract_task_node_attempt
    on forge_role_contract (task_id, node_id, attempt);

create index if not exists forge_role_contract_story_idx
    on forge_role_contract (story_id, created_at desc);

comment on table forge_role_contract is
    'One role decision per task/node/attempt, in FIELDS. Written by scripts/forge-handoff.mjs from the agent; read by the role runner, which prefers it over any JSON in the reply.';

commit;

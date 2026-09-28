-- CulebraLuxe Portal
-- FORGE work-order plan in ROWS — the last place a contract lived in JSON.
-- Migration: 171_forge_role_plan.sql
--
-- The decision moved to fields (170). The PLAN was the holdout: assignments with
-- chunks, surfaces, proofs and dependencies, shipped as a `LEAD_PLAN: {...}` line.
-- Same disease: a structured fact travelling as text that has to be parsed, with a
-- marker that can go missing and a brace that can go unbalanced inside a shell command.
--
-- So the plan is rows. A chunk is the atomic unit of work; an assignment groups the
-- chunks one worker owns. Every contract fact is a column with a real type, and the
-- database refuses the wrong shape at write time:
--
--   surface      text[]  NOT NULL, at least one entry — a chunk with no surface is
--                        a chunk that cannot be checked, so it never gets written
--   proof        text    NOT NULL, non-blank — "no proof" is not a plan
--   chunk_id     1..3    a fourth chunk is not "keep working", it is a recut
--   size         SMALL|MEDIUM only (a LARGE chunk is by definition un-decomposed)
--
-- features are the eight dispatchability numbers as REAL COLUMNS, not a JSON blob:
-- they are a numeric vector the dispatch gate multiplies, and a blob would have to be
-- parsed and trusted. Integers, range-checked.

begin;

create table if not exists forge_role_assignment (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    process_instance_id text not null,
    task_id text not null,
    node_id text not null,
    attempt integer not null default 1,

    assignment_id text not null,
    finding_ids text[] not null default '{}',
    evidence_refs text[] not null default '{}',
    reasoning text null,

    -- Dispatchability vector — the numbers the dispatch gate reads. Ranges match the
    -- reviewer's own validation exactly (two 1..100, six 1..5), so the database refuses
    -- whatever the gate would refuse: no value can be written that could not be routed.
    semantic_surface integer null check (semantic_surface is null or semantic_surface between 1 and 100),
    dependency_depth integer null check (dependency_depth is null or dependency_depth between 1 and 100),
    uncertainty integer null check (uncertainty is null or uncertainty between 1 and 5),
    context_burden integer null check (context_burden is null or context_burden between 1 and 5),
    proof_burden integer null check (proof_burden is null or proof_burden between 1 and 5),
    coupling integer null check (coupling is null or coupling between 1 and 5),
    change_novelty integer null check (change_novelty is null or change_novelty between 1 and 5),
    worker_fit integer null check (worker_fit is null or worker_fit between 1 and 5),

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists forge_role_assignment_key
    on forge_role_assignment (task_id, node_id, attempt, assignment_id);

create table if not exists forge_role_plan_chunk (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    process_instance_id text not null,
    task_id text not null,
    node_id text not null,
    attempt integer not null default 1,

    assignment_id text not null,
    chunk_id integer not null,
    size text null,
    surface text[] not null,
    proof text not null,
    invariant text null,
    preconditions text[] not null default '{}',
    postconditions text[] not null default '{}',
    classes text[] not null default '{}',
    risks text[] not null default '{}',
    depends_on text[] not null default '{}',

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint forge_role_plan_chunk_id_check check (chunk_id between 1 and 3),
    constraint forge_role_plan_chunk_size_check check (size is null or size in ('SMALL', 'MEDIUM')),
    constraint forge_role_plan_chunk_surface_check check (cardinality(surface) >= 1),
    constraint forge_role_plan_chunk_proof_check check (length(btrim(proof)) > 0)

);

create unique index if not exists forge_role_plan_chunk_key
    on forge_role_plan_chunk (task_id, node_id, attempt, assignment_id, chunk_id);

create index if not exists forge_role_plan_chunk_story_idx
    on forge_role_plan_chunk (story_id, created_at desc);

comment on table forge_role_assignment is
    'One worker-owned assignment per task/node/attempt/id, with the eight dispatchability numbers as columns. Written by scripts/forge-handoff.mjs --chunk; read by the role runner.';
comment on table forge_role_plan_chunk is
    'One serial chunk of work inside an assignment. surface/proof are NOT NULL: a chunk that cannot be checked is never written.';

commit;

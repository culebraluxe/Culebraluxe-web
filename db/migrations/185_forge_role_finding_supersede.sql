-- CulebraLuxe Portal
-- FORGE findings supersede — a later attempt cannot silently drop a seam the earlier one declared.
-- Migration: 185_forge_role_finding_supersede.sql
--
-- Migration 172 put findings in rows and made the reader take the NEWEST attempt per node. That
-- scope is correct — it is what stops a story-wide read resurrecting every earlier run and
-- feeding the Lead duplicate finding ids — but it also hid a loss: an attempt N that re-issued
-- its findings without a seam attempt N-1 had declared simply replaced the row, and the Lead
-- never learned the seam was gone. A silent drop is a defect, not a preference.
--
-- The writer (`scripts/forge-handoff.mjs --finding`) now computes the dropped seams before any
-- SQL. A write with no exact acknowledgement is REFUSED by name. A write that acknowledges the
-- loss with `--supersede <seam>` records one row here per dropped seam, naming the attempt and
-- the finding that declared it. So the two outcomes are distinguishable by these rows: a refusal
-- writes nothing, a supersede writes them.
--
-- One seam has one supersede record per (task, node, attempt); the unique index makes a repeated
-- acknowledgement idempotent rather than a second, disagreeing row.

begin;

create table if not exists forge_role_finding_supersede (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    process_instance_id text not null,
    task_id text not null,
    node_id text not null,
    attempt integer not null,

    finding_id text not null,
    seam text not null,
    declared_by_attempt integer not null,
    declared_by_finding_id text not null,

    created_at timestamptz not null default now(),

    constraint forge_role_finding_supersede_seam_check check (length(btrim(seam)) > 0),
    constraint forge_role_finding_supersede_finding_check check (length(btrim(finding_id)) > 0),
    constraint forge_role_finding_supersede_declared_check check (
        declared_by_attempt < attempt
    )
);

create unique index if not exists forge_role_finding_supersede_key
    on forge_role_finding_supersede (task_id, node_id, attempt, seam);

create index if not exists forge_role_finding_supersede_story_idx
    on forge_role_finding_supersede (story_id, created_at desc);

comment on table forge_role_finding_supersede is
    'One row per seam a later findings attempt explicitly superseded: the seam was declared by an earlier attempt (declared_by_attempt/declared_by_finding_id) for the same node and process instance and is absent from the attempt in force. Written by scripts/forge-handoff.mjs --finding --supersede; read by db/forge-role-finding.ts listStoryForgeFindingHandoff and stated in the Lead routing directive. A refusal writes no rows, so these rows are what distinguishes an explicit supersede from a refusal.';

commit;

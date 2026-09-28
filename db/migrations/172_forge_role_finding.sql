-- CulebraLuxe Portal
-- FORGE findings in ROWS — the last contract still travelling as chat JSON.
-- Migration: 172_forge_role_finding.sql
--
-- Lead and Smith are ripped: their decisions and plans are rows (170/171). FINDINGS are
-- the holdout. Architect and Scout still ship them as `FORGE_ARCHITECT_HANDOFF: {...}` or
-- `FORGE_FINDINGS_JSON: [...]` inside the reply, so a brace-balanced scan of chat is the
-- only way to learn what the Architect actually saw. A dropped marker costs a run; a
-- stray brace inside a quoted shell command truncates a plan.
--
-- This table is the edge, typed. Every fact is a column, and the database refuses the
-- shape the architect gate would refuse:
--
--   summary   NOT NULL, non-blank — a finding that states nothing is not a finding
--   seams     text[], 1..3 — the architect's own ceiling (MAX_SEAMS_PER_FINDING). A scope
--             with no legal path cannot be dispatched, so it is never stored
--   required  NOT NULL explicit — never inferred from the row's presence
--   hint      nullable, SAME_UNIT | SPLIT_CHILD | FOLLOW_UP_STORY | NOTE | HOLD
--   a REQUIRED HOLD must name a concrete risk (cardinality(risks) >= 1), which is the
--   same rule assessArchitectHandoff enforces, moved to the write door
--
-- The gate's rule and this table's rule are the same number on purpose: no row can be
-- written that the reviewer would have refused, and no refusal can be laundered by a row.
--
-- The reply parsers stay for now — Scout still ingests through FORGE_FINDINGS_JSON — and
-- they are fallback plus channel telemetry until this table is the only writer.

begin;

create table if not exists forge_role_finding (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    process_instance_id text not null,
    task_id text not null,
    node_id text not null,
    attempt integer not null default 1,

    finding_id text not null,
    summary text not null,
    required boolean not null default true,
    seams text[] not null,
    hint text null,
    preconditions text[] not null default '{}',
    postconditions text[] not null default '{}',
    classes text[] not null default '{}',
    risks text[] not null default '{}',

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint forge_role_finding_id_check check (length(btrim(finding_id)) > 0),
    constraint forge_role_finding_summary_check check (length(btrim(summary)) > 0),
    constraint forge_role_finding_seams_check check (cardinality(seams) between 1 and 3),
    constraint forge_role_finding_hint_check check (
        hint is null or hint in ('SAME_UNIT', 'SPLIT_CHILD', 'FOLLOW_UP_STORY', 'NOTE', 'HOLD')
    ),
    constraint forge_role_finding_hold_risk_check check (
        not (required and hint = 'HOLD' and cardinality(risks) = 0)
    )
);

create unique index if not exists forge_role_finding_key
    on forge_role_finding (task_id, node_id, attempt, finding_id);

create index if not exists forge_role_finding_story_idx
    on forge_role_finding (story_id, created_at desc);

comment on table forge_role_finding is
    'One Architect/Scout finding per task/node/attempt/finding_id, seams and risks as typed columns. Written by scripts/forge-handoff.mjs --finding; read by the role runner and the Lead context. Replaces reading FORGE_ARCHITECT_HANDOFF out of the reply.';

commit;

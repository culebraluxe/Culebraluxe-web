-- CulebraLuxe Portal
-- THE DISPATCH LEDGER: what we predicted about a unit of work, and what happened.
-- Migration: 175_forge_dispatch_score.sql
--
-- WHY THIS TABLE EXISTS
--
-- `forge-difficulty-scorer.ts` carries a hand-weighted logistic — p_success =
-- sigma(bias + w.x) over eight features — and its own header says the weights are
-- "provisional calibration food, not claims", to be replaced by a fit learned from run
-- history. The same file records that four of the eight features
-- (loc_ratio, context_ratio, historical_success, repo_size_bucket) are NEUTRAL DEFAULTS
-- because nothing captures them yet.
--
-- So the model has been predicting without a dataset: the plan-derived features, the
-- prediction and the gate verdict are computed at dispatch and thrown away, and the
-- outcome was never joined to the unit that was assessed. A regression cannot be fitted
-- against numbers nobody wrote down.
--
-- This ledger records BOTH halves, for every assessed unit:
--   * AT DISPATCH — the eight features, which of them were telemetry defaults rather
--     than measurements, the scorer identity, the logit components, p_success and the
--     gate verdict, bound to (story, process instance, task, node, attempt, assignment).
--   * AT COMPLETION — the observed outcome for that same unit: verdict, repairs, turns,
--     wall clock, cost, tokens, files changed and LOC delta where measured, plus the
--     candidate SHA. Null means not measured, never zero.
--
-- Additive and forward-only: no existing behaviour changes, no backfill is claimed, and
-- the model keeps its current weights until a fit earns better ones. Recording is the
-- precondition for that fit; making it the model's task tonight would be pretending.

begin;

create table if not exists forge_dispatch_score (
    id uuid primary key default gen_random_uuid(),

    -- identity of the unit that was assessed
    story_id text not null references storyboard_story(id) on delete cascade,
    process_instance_id text not null,
    task_id text not null,
    node_id text not null,
    attempt integer not null default 1,
    assignment_id text not null,
    -- 0 means "the assignment itself", not "chunk zero": a nullable column would make
    -- the identity unique index below unable to see the row it must prevent.
    chunk_id integer not null default 0,

    -- WHAT THE MODEL SAW (the quantitative vector, forge-difficulty-scorer.ts)
    files_touched integer not null,
    loc_ratio numeric(6,3) not null,
    dep_depth integer not null,
    has_acceptance boolean not null,
    context_ratio numeric(6,3) not null,
    historical_success numeric(6,3) not null,
    repo_size_bucket integer not null,
    generic_type boolean not null,
    -- Which of the eight were MEASURED for this plan and which were neutral defaults.
    -- Without this, a fit would treat a placeholder as evidence — the exact error the
    -- hand-set weights invite.
    measured_features text[] not null,

    -- WHAT THE MODEL SAID
    scorer_id text not null,
    logit numeric(10,4) not null,
    p_success numeric(6,4) not null check (p_success >= 0 and p_success <= 1),
    gate text not null check (gate in ('reject', 'flag', 'dispatch')),

    -- WHAT THE LEAD DECIDED ABOUT THIS UNIT (context for the fit)
    route text null check (route is null or route in ('SOLO', 'SMITH', 'SPLIT', 'HOLD')),

    -- WHAT HAPPENED (all nullable: unmeasured, never fabricated)
    outcome text null check (
        outcome is null or outcome in ('pass', 'fail', 'repair', 'hold', 'cancelled')
    ),
    repairs integer null,
    turns integer null,
    wall_ms integer null,
    cost_usd numeric(12,6) null,
    tokens_input integer null,
    tokens_output integer null,
    files_changed integer null,
    loc_delta integer null,
    candidate_sha text null,
    outcome_detail text null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint forge_dispatch_score_attempt_check check (attempt >= 1),
    constraint forge_dispatch_score_measured_check check (
        cardinality(measured_features) between 0 and 8
    )
);

-- ONE ROW PER ASSESSED UNIT per attempt. The recorder upserts here, so a re-scored
-- assignment updates its prediction instead of stacking duplicates.
create unique index if not exists forge_dispatch_score_key
    on forge_dispatch_score (task_id, node_id, attempt, assignment_id, chunk_id);

-- The fit reads by time and by story; both are in the predicate of any honest query.
create index if not exists forge_dispatch_score_time_idx
    on forge_dispatch_score (created_at desc);

create index if not exists forge_dispatch_score_story_idx
    on forge_dispatch_score (story_id, created_at desc);

-- An outcome arrives after the prediction, and the fit only uses rows that have both.
create index if not exists forge_dispatch_score_labeled_idx
    on forge_dispatch_score (outcome)
    where outcome is not null;

comment on table forge_dispatch_score is
    'Prediction-vs-outcome ledger for one assessed work unit: the eight difficulty features (with which were measured rather than defaulted), the logistic logit, p_success, the gate verdict, the Lead route, and the observed outcome. Written at dispatch by the role runner; the outcome is filled when the unit completes. The calibration set for replacing the hand-set weights in forge-difficulty-scorer.ts with a fit (ENG-FORGE-CALIBRATION-01).';

comment on column forge_dispatch_score.measured_features is
    'Subset of the eight feature names that were actually measured for this plan; the rest are the neutral telemetry defaults documented in forge-plan-difficulty.ts.';

commit;

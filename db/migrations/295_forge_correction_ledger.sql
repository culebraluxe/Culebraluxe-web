-- Forge correction ledger (Phase 3 learning, TD 3.3/3.4/3.5).
--
-- A durable, owner-reviewable record of corrections the lanes actually received, keyed so repeats
-- are detectable without a second learning store. The ledger stores what was corrected and, when
-- known, why; it does not store a "fix" until a human creates one. `permanent_rule_created` is the
-- link to the correction -> specification -> rail pipeline: the ledger row exists first, the rule
-- and its guard/test come later, and no free-text self-writer can promote itself.
--
-- `correction_key` is the PLAIN SHA-256 (hex) of the normalized correction text: trimmed,
-- internal whitespace collapsed to one space, lower-cased, hashed as UTF-8. No salt, no domain
-- prefix, no length prefix, so any SHA-256 tool reproduces a stored key from the text. The
-- correction text itself is stored VERBATIM in `correction`; normalization is only for the key. It is what repeat detection groups on: the same
-- correction requested for the same lane more than once is the signal Harness Evolution consumes
-- (`ImprovementSignal::RepeatedCorrection`), and it is not the same thing as the correction id.
--
-- Additive only; no existing table is altered. `experiment_id` optionally points at the Harness
-- Evolution experiment whose outcome produced the correction (migration 294), so an adopted/
-- rejected experiment writes back into Phase 3 without a parallel outcome table.
--
-- Deliberately NOT applied by the build lane: migrations are human holds (WO-07).

begin;

create table if not exists forge_correction_ledger (
    id bigserial primary key,
    -- The lane the correction was for (`lead`, `smith`, `assay`, `dev_ops`, a harness domain id, ...).
    lane text not null,
    -- Plain SHA-256 hex of the normalized correction text (see
    -- `db::forge_corrections::correction_key`): trim, collapse whitespace, lower-case, hash UTF-8.
    correction_key text not null,
    -- What the lane was told, verbatim.
    correction text not null,
    -- Where/when it applies: story id, run id, review note, `harness-experiment:<id>`, etc.
    context text,
    -- Why it happened, only when a reviewer stated one. NULL means not established, not "unknown why".
    root_cause text,
    -- The specification location the correction should become durable in (`docs/...`, xml node, rail).
    -- NULL until a human places it; the ledger does not invent a destination.
    spec_location text,
    -- True only after the correction has become a permanent rule with its guard/test.
    permanent_rule_created boolean not null default false,
    -- Durable Forge evidence ids this correction rests on (story run, hold, QA receipt, experiment).
    -- References, never copies.
    evidence_refs jsonb not null default '[]'::jsonb,
    -- The Harness Evolution experiment whose outcome produced this correction, when there is one.
    experiment_id text references forge_harness_experiment(experiment_id) on delete set null,
    created_at timestamptz not null default now(),
    check (length(trim(lane)) > 0),
    check (length(trim(correction)) > 0),
    check (length(correction_key) = 64)
);

-- Repeat detection: same lane, same normalized correction, newest first.
create index if not exists idx_forge_correction_ledger_repeat
    on forge_correction_ledger (lane, correction_key, created_at desc);

-- Operator/CLI reads by lane, newest first.
create index if not exists idx_forge_correction_ledger_lane
    on forge_correction_ledger (lane, created_at desc);

-- "What corrections have not become rules yet" — the correction -> rail backlog.
create index if not exists idx_forge_correction_ledger_open
    on forge_correction_ledger (lane, created_at desc)
    where permanent_rule_created = false;

create index if not exists idx_forge_correction_ledger_experiment
    on forge_correction_ledger (experiment_id)
    where experiment_id is not null;

comment on table forge_correction_ledger is
    'Durable correction ledger for Phase 3 learning: what each lane was corrected on, keyed for repeat detection. Additive to Forge evidence; no second learning store.';
comment on column forge_correction_ledger.correction_key is
    'Plain SHA-256 hex of the normalized correction text (trim, whitespace collapsed to one space, lower-cased UTF-8; no salt/domain/length prefix). Same lane + same key recurring is a repeated correction. The correction column stores the text verbatim.';
comment on column forge_correction_ledger.permanent_rule_created is
    'True only after a human turned this correction into a permanent rule plus its guard/test; the ledger never self-promotes.';
comment on column forge_correction_ledger.evidence_refs is
    'Durable Forge evidence ids (story run, hold, QA receipt, experiment). References, not a telemetry copy.';
comment on column forge_correction_ledger.experiment_id is
    'Harness Evolution experiment (migration 294) whose adopted/rejected outcome produced this correction, when applicable.';

commit;

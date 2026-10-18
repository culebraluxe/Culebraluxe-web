-- Harness Evolution — durable revision catalog, experiments, probes and the active pointer.
--
-- Additive only. Nothing here duplicates measurements that Forge already stores: a probe row
-- *references* the durable evidence it rests on (`process_instances`, `storyboard_story_run`,
-- `forge_engine_task_execution`, evidence ids in jsonb) instead of copying telemetry. The optimizer
-- has no execution, publication or deployment table here: adoption is the `forge_harness_activation`
-- pointer, nothing more.
--
-- The active pointer is enforced by the schema, not by convention: a partial unique index allows at
-- most one `is_current` activation row per domain, and an activation row references the revision it
-- made active. A rollback writes a new activation row pointing at the exact prior revision.
--
-- Lifecycle strings match `model.rs::RevisionStatus`; component kinds/authority match `model.rs`.
-- Do not run this against a database from the build lane; it is applied by the migration pipeline.

begin;

-- One immutable, content-addressed harness revision. `manifest_hash` is the stable hash of
-- `manifest`; a reader recomputes it and refuses a mismatch. The parent link is what makes lineage
-- an audit trail rather than a claim.
create table if not exists forge_harness_revision (
    revision_id text primary key,
    parent_revision_id text references forge_harness_revision(revision_id) on delete restrict,
    domain text not null,
    status text not null check (status in (
        'draft', 'proposed', 'probe_ready', 'probing', 'compared', 'composed', 'validating',
        'adoptable', 'adopted', 'rejected', 'failed', 'superseded', 'rolled_back'
    )),
    manifest jsonb not null,
    manifest_hash text not null,
    created_at timestamptz not null default now(),
    created_by text not null,
    check (length(trim(revision_id)) > 0),
    check (length(trim(manifest_hash)) = 64)
);

create index if not exists idx_forge_harness_revision_domain
    on forge_harness_revision (domain, created_at desc);

create index if not exists idx_forge_harness_revision_parent
    on forge_harness_revision (parent_revision_id);

create index if not exists idx_forge_harness_revision_status
    on forge_harness_revision (domain, status);

-- The eight editable surfaces of one revision. The manifest jsonb stays authoritative for hashing;
-- this table is the readable projection used by review/listing, keyed exactly one row per kind.
create table if not exists forge_harness_component (
    revision_id text not null references forge_harness_revision(revision_id) on delete cascade,
    kind text not null check (kind in (
        'system_prompt', 'guardrails', 'memory', 'tool_guidance',
        'subagents', 'skills', 'scripts', 'plugins'
    )),
    authority text not null check (authority in ('green', 'yellow', 'red')),
    content text not null,
    description text not null default '',
    primary key (revision_id, kind)
);

-- One Propose–Probe–Compose–Validate pass against a pinned base revision. `base_manifest_hash` is
-- pinned at creation so a base that changed underneath is detectable without trusting the caller.
create table if not exists forge_harness_experiment (
    experiment_id text primary key,
    domain text not null,
    base_revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    base_manifest_hash text not null,
    mode text not null check (mode in ('parallel', 'sequential')),
    k integer not null default 1 check (k >= 1),
    max_steps integer not null default 1 check (max_steps >= 1),
    max_probes integer not null default 3 check (max_probes >= 1),
    seed bigint not null default 0,
    state text not null check (state in (
        'draft', 'proposed', 'probe_ready', 'probing', 'compared', 'composed', 'validating',
        'adoptable', 'adopted', 'rejected', 'failed', 'superseded', 'rolled_back'
    )),
    failure_stage text check (failure_stage in (
        'propose', 'policy', 'probe', 'contrast', 'compose', 'validate'
    )),
    failure_code text,
    failure_message text,
    successor_revision_id text references forge_harness_revision(revision_id) on delete set null,
    -- ValidationEvidence: the four gate statuses plus evidence refs. A behavioral contrast alone is
    -- not sufficient to adopt, so the gates are durable on the experiment row.
    validation jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (max_probes >= 1)
);

create index if not exists idx_forge_harness_experiment_domain
    on forge_harness_experiment (domain, created_at desc);

create index if not exists idx_forge_harness_experiment_state
    on forge_harness_experiment (state);

create index if not exists idx_forge_harness_experiment_base
    on forge_harness_experiment (base_revision_id);

-- Invariant and domain candidates are stored separately (proposal_kind) until composition. Each
-- candidate materializes as a child revision; that revision id is recorded here, not duplicated.
create table if not exists forge_harness_candidate (
    candidate_id text not null,
    experiment_id text not null references forge_harness_experiment(experiment_id) on delete cascade,
    base_revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    proposal_kind text not null check (proposal_kind in ('invariant', 'domain')),
    changes jsonb not null,
    candidate_revision_id text references forge_harness_revision(revision_id) on delete set null,
    created_at timestamptz not null default now(),
    primary key (experiment_id, candidate_id)
);

create index if not exists idx_forge_harness_candidate_revision
    on forge_harness_candidate (candidate_revision_id);

-- One pinned probe execution. The probe identity is the contract digest over the full pin set
-- (task, model, model policy, executor, tools, environment class, input dataset, mode, k, seed).
-- Evidence is referenced, never copied: evidence_refs holds durable Forge evidence ids, and the
-- optional FKs point at the execution/run rows that produced them where those exist.
create table if not exists forge_harness_probe (
    probe_id text primary key,
    experiment_id text not null references forge_harness_experiment(experiment_id) on delete cascade,
    candidate_revision_id text references forge_harness_revision(revision_id) on delete set null,
    side text not null check (side in ('reference', 'candidate')),
    revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    contract_digest text not null,
    spec jsonb not null,
    evidence_id text,
    evidence_refs jsonb not null default '[]'::jsonb,
    process_instance_id uuid references process_instances(id) on delete set null,
    story_run_id uuid references storyboard_story_run(id) on delete set null,
    engine_task_id uuid references forge_engine_task_execution(task_id) on delete set null,
    created_at timestamptz not null default now(),
    check (length(trim(contract_digest)) = 64)
);

create index if not exists idx_forge_harness_probe_experiment
    on forge_harness_probe (experiment_id, side);

create index if not exists idx_forge_harness_probe_engine_task
    on forge_harness_probe (engine_task_id)
    where engine_task_id is not null;

-- Behavioral contrast over one matched reference/candidate probe pair. Identity mismatch
-- invalidates the report; the jsonb carries per-dimension direction and evidence refs.
create table if not exists forge_harness_contrast (
    contrast_id text primary key,
    experiment_id text not null references forge_harness_experiment(experiment_id) on delete cascade,
    base_revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    candidate_revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    reference_probe_id text references forge_harness_probe(probe_id) on delete set null,
    candidate_probe_id text references forge_harness_probe(probe_id) on delete set null,
    report jsonb not null,
    evidence_refs jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now()
);

create index if not exists idx_forge_harness_contrast_experiment
    on forge_harness_contrast (experiment_id);

-- The composition decision: per-component Keep/Adopt/Reject/Merge plus the successor manifest hash.
-- The successor revision is a new child row; the parent revision is never updated in place.
create table if not exists forge_harness_composition (
    composition_id text primary key,
    experiment_id text not null references forge_harness_experiment(experiment_id) on delete cascade,
    base_revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    successor_revision_id text references forge_harness_revision(revision_id) on delete set null,
    successor_manifest_hash text not null,
    decision jsonb not null,
    created_at timestamptz not null default now()
);

create unique index if not exists forge_harness_composition_experiment
    on forge_harness_composition (experiment_id);

-- Domain-scoped experience library. Scope is domain + executor + mode + budget + seed, matching
-- ExperienceScope; a lesson needs a supporting contrast and a completed validation to exist.
create table if not exists forge_harness_experience (
    experience_id text primary key,
    domain text not null,
    executor text not null,
    mode text not null check (mode in ('parallel', 'sequential')),
    k integer not null check (k >= 1),
    max_steps integer not null check (max_steps >= 1),
    max_probes integer not null check (max_probes >= 1),
    seed bigint not null default 0,
    harness_revision_id text references forge_harness_revision(revision_id) on delete set null,
    manifest_hash text not null,
    lesson text not null,
    applicability jsonb not null default '[]'::jsonb,
    counterevidence jsonb not null default '[]'::jsonb,
    confidence integer not null check (confidence between 0 and 100),
    contrast_id text references forge_harness_contrast(contrast_id) on delete restrict,
    validation jsonb not null,
    evidence_refs jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now(),
    expires_at timestamptz,
    check (length(trim(lesson)) > 0)
);

create index if not exists idx_forge_harness_experience_scope
    on forge_harness_experience (domain, executor, mode, seed, created_at desc);

create index if not exists idx_forge_harness_experience_contrast
    on forge_harness_experience (contrast_id);

-- The active-harness pointer. In one transaction, the previous `is_current` row is cleared and a
-- new current row is inserted; the partial unique index makes "two active harnesses for one domain"
-- unrepresentable. A rollback is an activation row whose `to_revision_id` is the exact revision
-- that was current before.
create table if not exists forge_harness_activation (
    activation_id bigserial primary key,
    domain text not null,
    sequence bigint not null,
    activation_kind text not null check (activation_kind in ('adopt', 'rollback')),
    from_revision_id text references forge_harness_revision(revision_id) on delete set null,
    to_revision_id text not null references forge_harness_revision(revision_id) on delete restrict,
    decision_id text not null,
    activated_by text not null,
    activated_at timestamptz not null default now(),
    is_current boolean not null default true
);

create unique index if not exists forge_harness_activation_one_current
    on forge_harness_activation (domain)
    where is_current;

create index if not exists idx_forge_harness_activation_domain
    on forge_harness_activation (domain, activated_at desc);

create index if not exists idx_forge_harness_activation_to_revision
    on forge_harness_activation (to_revision_id);

comment on table forge_harness_revision is
    'Immutable harness revisions. A row is never updated in place; a change is a new child revision with a new manifest_hash.';
comment on column forge_harness_revision.manifest_hash is
    'Stable SHA-256 of the canonical manifest (model.rs::HarnessManifest::manifest_hash); readers recompute and refuse a mismatch.';
comment on table forge_harness_experiment is
    'One Propose-Probe-Contrast-Compose-Validate pass against a pinned base revision. State follows model.rs::RevisionStatus. A failed experiment never changes forge_harness_activation.';
comment on column forge_harness_experiment.validation is
    'ValidationEvidence (static_policy/regression_rails/replay/phase3_evidence gate statuses + evidence refs). A behavioral contrast alone is not enough to adopt.';
comment on table forge_harness_probe is
    'Pinned probe executions. evidence_refs and the optional process/run/engine-task FKs point at existing Forge evidence; no telemetry is copied here.';
comment on table forge_harness_contrast is
    'Behavioral contrast between one matched reference/candidate probe pair; an identity mismatch invalidates the report.';
comment on table forge_harness_composition is
    'Per-component composition decision and the successor manifest hash; the successor is a new revision row, the parent is never overwritten.';
comment on table forge_harness_experience is
    'Domain-scoped experience library: domain + executor + mode + budget + seed. Insert requires a supporting contrast and completed validation.';
comment on table forge_harness_activation is
    'Atomic active-harness pointer per domain: at most one is_current row per domain; rollback writes a new row to the exact prior revision.';

commit;

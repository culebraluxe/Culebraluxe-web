-- Trajectory telemetry, turn-keyed. 292 keyed one row per (Story Run, harness profile), which cannot tell a role's
-- FIRST turn from its repair turn, a REPLAN's turn from the one before it, or a retried settlement from a second
-- attempt: every same-profile turn of one run collapsed into one row and each overwrote the last. Measurement is
-- only worth recording if a reader can say WHICH turn was measured, so this migration puts the identity on the row.
--
-- The identity is the model-attempt identity the spend ledger already uses, not a new one: `attempt_key` is unique
-- within a run, and `(generation_id, task_id, role_attempt)` names the same durable attempt that
-- `forge_model_generation_attempt` (286) holds spend for, so a reader that asks for the settled receipt can join it
-- to a turn. Nothing below joins it, and the turn's own row stays the one home for the reading this turn reported.
-- `attempt_key` is composed so that a REPLAN — a new generation for the same task and role attempt — does not
-- overwrite the previous turn's row.
--
-- UNKNOWN IS NOT ZERO, and it is not NULL-by-accident either. Every measurement column added here is nullable on
-- purpose, and the flag beside them is three-valued on purpose: NULL means no reading was ever taken (or the row
-- predates turns), `usage_known = false` means the turn ended without a reading, and `usage_known = true` means the
-- numbers beside it are the reading the harness reported for this turn. A reader must not coerce any of the three
-- into the others — an unpriced turn is not a free one.
--
-- The QA verdict is deliberately NOT copied onto these rows. A turn row is written when the turn ends; QA runs
-- later, and sometimes never. The verdict is therefore joined at read time from `forge_workflow_evidence` (109) and
-- only when `qa_verified_sha` names the same candidate the turn produced — a verdict for a different candidate is
-- not a verdict for this one.
begin;

alter table storyboard_run_trajectory
    add column if not exists attempt_key text,
    add column if not exists generation_id uuid,
    add column if not exists task_id text,
    add column if not exists role_attempt integer,
    add column if not exists workflow_node text,
    add column if not exists role text,
    add column if not exists profile_version integer,
    add column if not exists model text,
    add column if not exists provider text,
    add column if not exists turn_started_at timestamptz,
    add column if not exists turn_ended_at timestamptz,
    add column if not exists outcome text,
    add column if not exists first_edit_latency_ms bigint,
    add column if not exists tokens_input bigint,
    add column if not exists tokens_output bigint,
    add column if not exists cost_usd numeric(12, 6),
    add column if not exists usage_known boolean,
    add column if not exists candidate_sha text;

-- Rows written under 292 belong to a run and a profile but not to a tellable turn. `legacy:` is a marker, not a
-- claimed attempt key: it says "written before turns were distinguished" and it keeps every measurement those rows
-- carry exactly as it was, unknown where it was unknown. A second write for the same run and profile still replaces
-- such a row, which is all 292 ever promised.
update storyboard_run_trajectory
   set attempt_key = 'legacy:' || harness_profile_id
 where attempt_key is null;

alter table storyboard_run_trajectory
    alter column attempt_key set not null;

alter table storyboard_run_trajectory
    drop constraint if exists storyboard_run_trajectory_pkey;

alter table storyboard_run_trajectory
    add primary key (run_id, attempt_key);

-- For a reader that later asks the spend ledger for a turn's settled receipt, which is keyed on exactly this triple.
create index if not exists storyboard_run_trajectory_attempt_join
    on storyboard_run_trajectory (generation_id, task_id, role_attempt);

comment on column storyboard_run_trajectory.attempt_key is
    'Stable execution key of this turn within its run: unique per run, stable across a retry of the same settlement, distinct for each role attempt and each REPLAN generation. Rows written before 293 carry ''legacy:<profile>''.';
comment on column storyboard_run_trajectory.harness_profile_id is
    'The harness profile the turn ran under. Kept beside the turn key: a run can hold several turns of one profile.';
comment on column storyboard_run_trajectory.generation_id is
    'The paid model generation this turn belongs to (forge_model_generation_attempt.generation_id) — the run''s execution generation, new after a REPLAN. Null = no durable attempt controller (hand-run or memory lane).';
comment on column storyboard_run_trajectory.role_attempt is
    'This turn''s attempt index within its role: 0 is the first turn, N>0 is a repair/reprompt turn. Matches forge_model_attempt.role_attempt.';
comment on column storyboard_run_trajectory.first_edit_latency_ms is
    'Milliseconds from the turn''s start to its first edit, in the vendor clock the tool-call timestamps use. Null = the turn never edited, or the start was not observed.';
comment on column storyboard_run_trajectory.usage_known is
    'true = the token/cost columns are the reading the harness reported for THIS turn; false = the turn ended without a reading; null = the row carries no reading at all (written before turns were recorded, or by a path that captured none). false and null are not zero.';
comment on column storyboard_run_trajectory.outcome is
    'How the turn ended: completed, failed, cancelled (interrupted) or uncertain (engine fault). Null on rows written before 293.';
comment on column storyboard_run_trajectory.candidate_sha is
    'The candidate this turn produced, when it produced one. Joined at read time against forge_workflow_evidence.qa_verified_sha to find the verdict for THIS candidate.';

commit;

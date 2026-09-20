-- CIL-02  Council session + verdict
-- Rename this file to the next free NNN at implement time.
-- Writer: db/council.ts only. See docs/agent/COLUMN-WRITER-AUDIT.md.

create table if not exists council_vendor_pin (
  id                text primary key default 'singleton',
  repo_url          text not null,
  git_sha           text not null,
  tag               text,
  adapter_version   text not null,
  protocol_files    jsonb not null default '[]'::jsonb,
  recorded_at       timestamptz not null default now(),
  constraint council_vendor_pin_singleton check (id = 'singleton')
);

create table if not exists council_session (
  id                uuid primary key default gen_random_uuid(),
  subject_type      text not null,
  subject_id        text not null,
  domain            text not null,
  mode              text not null,
  question          text not null,
  panel             jsonb not null,
  status            text not null,
  refuse_reason     text,
  vendor_sha        text not null,
  adapter_version   text not null,
  correlation_id    text not null,
  actor_kind        text not null,
  actor_id          text,
  created_at        timestamptz not null default now(),
  updated_at        timestamptz not null default now(),
  constraint council_session_subject_type_chk
    check (subject_type in (
      'deal', 'offer', 'match', 'listing_intake',
      'forge_story', 'forge_architect', 'forge_lead_pre', 'paper_intake'
    )),
  constraint council_session_domain_chk
    check (domain in ('app', 'forge')),
  constraint council_session_mode_chk
    check (mode in ('refuse', 'duo', 'quick')),
  constraint council_session_status_chk
    check (status in ('queued', 'running', 'verdict', 'refused', 'failed')),
  constraint council_session_actor_kind_chk
    check (actor_kind in ('user', 'system', 'agent')),
  constraint council_session_refuse_reason_chk
    check (
      (status = 'refused' and refuse_reason is not null)
      or (status <> 'refused')
    )
);

comment on table council_session is
  'Council convene record. One writer: services/council via db/council.ts.';

-- At most one in-flight session per subject. Completed sessions may stack;
-- getBySubject reads the latest.
create unique index if not exists council_session_inflight_uq
  on council_session (subject_type, subject_id)
  where status in ('queued', 'running');

create index if not exists council_session_subject_idx
  on council_session (subject_type, subject_id, created_at desc);

create index if not exists council_session_correlation_idx
  on council_session (correlation_id);

create table if not exists council_verdict (
  id                uuid primary key default gen_random_uuid(),
  session_id        uuid not null unique
                      references council_session(id) on delete restrict,
  pick              text not null,
  reasons           jsonb not null,
  dissent           jsonb not null,
  kill              jsonb not null,
  next_action       text not null,
  next_owner        text,
  review_at         timestamptz,
  evidence          jsonb not null,
  outcome           text,
  outcome_at        timestamptz,
  outcome_note      text,
  raw_model         jsonb,
  created_at        timestamptz not null default now(),
  constraint council_verdict_pick_chk
    check (char_length(pick) between 1 and 200),
  constraint council_verdict_reasons_chk
    check (jsonb_typeof(reasons) = 'array' and jsonb_array_length(reasons) between 1 and 3),
  constraint council_verdict_dissent_chk
    check (jsonb_typeof(dissent) = 'array' and jsonb_array_length(dissent) >= 1),
  constraint council_verdict_kill_chk
    check (jsonb_typeof(kill) = 'array' and jsonb_array_length(kill) >= 1),
  constraint council_verdict_evidence_chk
    check (jsonb_typeof(evidence) = 'array'),
  constraint council_verdict_outcome_chk
    check (
      outcome is null
      or outcome in ('confirmed', 'revised', 'reversed', 'inconclusive')
    ),
  constraint council_verdict_outcome_at_chk
    check (
      (outcome is null and outcome_at is null)
      or (outcome is not null and outcome_at is not null)
    )
);

comment on table council_verdict is
  'Single verdict for a council session. Checkpoint updates outcome columns only.';

create index if not exists council_verdict_review_idx
  on council_verdict (review_at)
  where outcome is null and review_at is not null;

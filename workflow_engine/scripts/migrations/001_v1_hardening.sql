-- ============================================================
-- Workflow engine V1 hardening (CRM-14 enabling extensions)
-- Generated; not yet applied. Run manually against the engine's
-- own Postgres database (never the CulebraLuxe schema).
-- ============================================================

-- 1. Process terminal outcome + optional application subject.
ALTER TABLE process_instances
  ADD COLUMN outcome text
    CHECK (outcome IN ('completed', 'cancelled', 'failed', 'conflict')),
  ADD COLUMN subject_type text,
  ADD COLUMN subject_id text;

-- 2. Token branch outcome + required/optional branch flag.
ALTER TABLE tokens
  ADD COLUMN outcome text
    CHECK (outcome IN ('completed', 'cancelled', 'failed', 'skipped')),
  ADD COLUMN required boolean NOT NULL DEFAULT true;

CREATE INDEX idx_tokens_parent_outcome
  ON tokens (parent_token_id, status, required)
  WHERE status = 'active';

-- 3. Stable application-command execution records (engine-side idempotency).
CREATE TABLE process_commands (
  id                  bigserial PRIMARY KEY,
  process_instance_id uuid NOT NULL REFERENCES process_instances(id) ON DELETE CASCADE,
  token_id            uuid,
  node_id             text NOT NULL,
  visit_sequence      integer NOT NULL DEFAULT 1,
  command_id          text NOT NULL,
  command_type        text NOT NULL,
  subject_type        text,
  subject_id          text,
  correlation_id      text,
  causation_id        text,
  input               jsonb NOT NULL DEFAULT '{}',
  outcome             text NOT NULL,
  message             text,
  created_at          timestamptz NOT NULL DEFAULT now(),
  UNIQUE (process_instance_id, node_id, visit_sequence),
  UNIQUE (command_id)
);

CREATE INDEX idx_process_commands_instance ON process_commands (process_instance_id);

-- =====================================================
-- Lightweight Workflow Engine Schema (Neon / Postgres)
-- Inspired by classic jBPM 3 token model
-- =====================================================

-- 1. Process Definitions (immutable versions)
CREATE TABLE process_definitions (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id       uuid,
  key             text NOT NULL,
  version         integer NOT NULL DEFAULT 1,
  name            text NOT NULL,
  description     text,
  definition      jsonb NOT NULL,
  status          text NOT NULL DEFAULT 'active'
                  CHECK (status IN ('draft', 'active', 'deprecated')),
  created_at      timestamptz NOT NULL DEFAULT now(),
  created_by      text,
  updated_at      timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, key, version)
);

CREATE INDEX idx_process_definitions_tenant_key ON process_definitions (tenant_id, key);

-- 2. Process Instances
CREATE TABLE process_instances (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid,
  definition_id       uuid NOT NULL REFERENCES process_definitions(id),
  business_key        text,
  status              text NOT NULL
                      CHECK (status IN ('active', 'completed', 'suspended', 'aborted', 'error')),
  started_at          timestamptz NOT NULL DEFAULT now(),
  ended_at            timestamptz,
  started_by          text,
  parent_instance_id  uuid REFERENCES process_instances(id),
  root_token_id       uuid,
  variables           jsonb NOT NULL DEFAULT '{}',
  version             integer NOT NULL DEFAULT 1,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_process_instances_tenant ON process_instances (tenant_id);
CREATE INDEX idx_process_instances_definition ON process_instances (definition_id);
CREATE INDEX idx_process_instances_status ON process_instances (status) WHERE status = 'active';
CREATE INDEX idx_process_instances_business_key ON process_instances (tenant_id, business_key);

-- 3. Tokens (hierarchical)
CREATE TABLE tokens (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid,
  process_instance_id uuid NOT NULL REFERENCES process_instances(id) ON DELETE CASCADE,
  parent_token_id     uuid REFERENCES tokens(id),
  node_id             text NOT NULL,
  status              text NOT NULL DEFAULT 'active'
                      CHECK (status IN ('active', 'completed', 'suspended')),
  is_able_to_reactivate_parent boolean NOT NULL DEFAULT true,
  started_at          timestamptz NOT NULL DEFAULT now(),
  ended_at            timestamptz,
  version             integer NOT NULL DEFAULT 1,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_tokens_instance ON tokens (process_instance_id);
CREATE INDEX idx_tokens_parent ON tokens (parent_token_id);
CREATE INDEX idx_tokens_status ON tokens (process_instance_id, status) WHERE status = 'active';

-- 4. Human Tasks
CREATE TABLE tasks (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid,
  process_instance_id uuid NOT NULL REFERENCES process_instances(id) ON DELETE CASCADE,
  token_id            uuid REFERENCES tokens(id),
  name                text NOT NULL,
  description         text,
  status              text NOT NULL DEFAULT 'created'
                      CHECK (status IN (
                        'created', 'ready', 'reserved', 'in_progress',
                        'completed', 'failed', 'exited', 'obsolete'
                      )),
  assignee            text,
  candidates          text[] DEFAULT '{}',
  swimlane            text,
  priority            integer NOT NULL DEFAULT 0,
  due_date            timestamptz,
  form_key            text,
  form_data           jsonb NOT NULL DEFAULT '{}',
  created_at          timestamptz NOT NULL DEFAULT now(),
  claimed_at          timestamptz,
  completed_at        timestamptz,
  completed_by        text,
  version             integer NOT NULL DEFAULT 1,
  updated_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_tasks_assignee_status ON tasks (assignee, status)
  WHERE status IN ('ready', 'reserved', 'in_progress');
CREATE INDEX idx_tasks_candidates ON tasks USING GIN (candidates);
CREATE INDEX idx_tasks_instance ON tasks (process_instance_id);
CREATE INDEX idx_tasks_due ON tasks (due_date) WHERE status IN ('ready', 'reserved', 'in_progress');

-- 5. Jobs / Timers / Async
CREATE TABLE jobs (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid,
  process_instance_id uuid REFERENCES process_instances(id) ON DELETE CASCADE,
  token_id            uuid REFERENCES tokens(id),
  type                text NOT NULL,
  due_at              timestamptz NOT NULL,
  status              text NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending', 'locked', 'completed', 'failed', 'cancelled')),
  locked_by           text,
  locked_until        timestamptz,
  attempts            integer NOT NULL DEFAULT 0,
  max_attempts        integer NOT NULL DEFAULT 5,
  payload             jsonb NOT NULL DEFAULT '{}',
  last_error          text,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now(),
  completed_at        timestamptz
);

CREATE INDEX idx_jobs_due_pending ON jobs (due_at) WHERE status = 'pending';
CREATE INDEX idx_jobs_locked ON jobs (locked_until) WHERE status = 'locked';
CREATE INDEX idx_jobs_instance ON jobs (process_instance_id);

-- 6. Process Events (append-only, partition-ready)
CREATE TABLE process_events (
  id                  bigserial,
  tenant_id           uuid,
  process_instance_id uuid NOT NULL,
  token_id            uuid,
  task_id             uuid,
  job_id              uuid,
  event_type          text NOT NULL,
  node_id             text,
  actor               text,
  data                jsonb NOT NULL DEFAULT '{}',
  created_at          timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

-- Example partitions (create more as needed)
CREATE TABLE process_events_2026_08 PARTITION OF process_events
  FOR VALUES FROM ('2026-08-01') TO ('2026-09-01');
CREATE TABLE process_events_2026_09 PARTITION OF process_events
  FOR VALUES FROM ('2026-09-01') TO ('2026-10-01');
CREATE TABLE process_events_2026_10 PARTITION OF process_events
  FOR VALUES FROM ('2026-10-01') TO ('2026-11-01');

CREATE INDEX idx_process_events_instance ON process_events (process_instance_id, created_at);
CREATE INDEX idx_process_events_type ON process_events (event_type, created_at);

-- Auto-update updated_at
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_process_instances_updated
  BEFORE UPDATE ON process_instances
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_tokens_updated
  BEFORE UPDATE ON tokens
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_tasks_updated
  BEFORE UPDATE ON tasks
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_jobs_updated
  BEFORE UPDATE ON jobs
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();

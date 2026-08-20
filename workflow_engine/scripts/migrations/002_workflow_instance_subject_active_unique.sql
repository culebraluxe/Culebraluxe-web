-- ============================================================
-- Workflow engine V1 — one active instance per workflow/context.
--
-- Enforces that at most one ACTIVE process instance exists for a given
-- (definition, subject_type, subject_id) — e.g. one active
-- transaction-close-v1 instance per deal — WITHOUT globally serializing every
-- workflow attached to a deal. Two different workflow definitions may be
-- active for the same deal at the same time.
--
-- `definition_id` identifies the versioned process definition row, so
-- uniqueness is scoped to the exact workflow definition the instance started
-- with. Logical workflow identity (process_definitions.key) is intentionally
-- not encoded here: the engine instance carries definition_id (version-pinned),
-- and process_definitions has no key column to index across. The application
-- start boundary pins one definition version (transaction-close-v1 v1), so
-- version drift cannot silently create duplicate instances of the same
-- workflow.
--
-- Schema-only infrastructure; no engine runtime changes.
-- ============================================================

create unique index process_instances_definition_subject_active_unique
  on process_instances (definition_id, subject_type, subject_id)
  where status = 'active';

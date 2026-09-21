-- Scope C review park — Architect completion may STOP for human review (reusing
-- the existing `hold` wait) before implementation. Default-off additive boolean.
alter table forge_workflow_evidence
  add column if not exists architecture_review_required boolean;

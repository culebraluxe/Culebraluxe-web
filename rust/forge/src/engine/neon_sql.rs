//! SQL the process binary issues against the EXISTING Neon tables.
//! Copied from `db/workflow-command-receipt.ts` and `db/forge-workflow-evidence.ts`.
//! Do not invent columns. Do not add a second receipt table.

pub const CLAIM_RECEIPT: &str = "\
INSERT INTO workflow_command_receipt (command_id, outcome, aggregate_id, message, actor_app_user_id)
VALUES ($1, 'pending', NULL, NULL, $2)
ON CONFLICT (command_id) DO NOTHING
RETURNING command_id";

pub const FINALIZE_RECEIPT: &str = "\
UPDATE workflow_command_receipt
   SET outcome = $2, aggregate_id = $3, message = $4, actor_app_user_id = $5
 WHERE command_id = $1";

pub const READ_RECEIPT: &str = "\
SELECT command_id, outcome, aggregate_id, message, actor_app_user_id
  FROM workflow_command_receipt
 WHERE command_id = $1
 LIMIT 1";

pub const RECEIPT_WATERMARK: &str = "\
SELECT max(created_at) AS watermark
  FROM workflow_command_receipt
 WHERE command_id LIKE $1";

pub const READ_EVIDENCE: &str = "\
SELECT e.*
  FROM forge_workflow_evidence e
  JOIN process_instances pi ON pi.id = e.process_instance_id
 WHERE e.story_id = $1
 ORDER BY (pi.status = 'active') DESC, e.updated_at DESC
 LIMIT 1";

pub const STORY_LEDGER: &str = "\
SELECT forge_repair_attempts, forge_replan_attempts, forge_last_qa_disposition
  FROM storyboard_story
 WHERE id = $1";

/// Increment observers on the canonical story row (repair_smith / repair_architect).
pub const INC_REPAIR: &str = "\
UPDATE storyboard_story
   SET forge_repair_attempts = coalesce(forge_repair_attempts, 0) + 1
 WHERE id = $1";

pub const INC_REPLAN: &str = "\
UPDATE storyboard_story
   SET forge_replan_attempts = coalesce(forge_replan_attempts, 0) + 1
 WHERE id = $1";

pub const RECEIPT_PREFIX: &str = "forge.completion:";

pub const READ_STORY_PACKET: &str = "\
SELECT id, title, goal, architect_brief, acceptance_criteria, assay_commands, context_refs
  FROM storyboard_story
 WHERE id = $1
 LIMIT 1";

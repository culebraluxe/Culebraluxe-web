# Forge legacy completion receipt reconciliation

## Why a stale pending receipt is quarantined

The current completion protocol inserts its `pending` receipt, applies the repair/replan counter and evidence
effects, and marks the receipt successful in one PostgreSQL transaction. A committed row cannot remain `pending`
under that protocol. A persisted stale pending row therefore belongs to the pre-atomic path (or is otherwise
anomalous); its age proves only that the owner stopped, not that no counter or evidence effect was written.
Forge changes such a row to `quarantined` and does not replay it. Quarantined rows do not count as final receipts
and do not advance the completion-reconciliation watermark, so retry continues to fail closed and surfaces the
case for an operator.

## Inspect before resolving

Use a read-only transaction first. Replace the command ID with the exact `forge.completion:<task UUID>` value.

```sql
begin read only;

select command_id, outcome, request_fingerprint, message, aggregate_id,
       created_at, updated_at, result_payload
  from workflow_command_receipt
 where command_id = 'forge.completion:<task UUID>';

select pe.id, pe.process_instance_id, pe.task_id, pe.node_id, pe.data,
       pi.subject_id, pi.business_key
  from process_events pe
  join process_instances pi on pi.id = pe.process_instance_id
 where pe.event_type = 'task.completed'
   and pe.task_id::text = '<task UUID>'
 order by pe.id;

select id, forge_repair_attempts, forge_replan_attempts
  from storyboard_story
 where id = '<story ID>';

select process_instance_id, story_id, work_type, qa_passed, candidate_sha,
       qa_verified_sha, published_sha, updated_at
  from forge_workflow_evidence
 where process_instance_id = '<process instance UUID>';

rollback;
```

Compare the accepted event payload, its node/story/instance, the corresponding evidence row, and the relevant
counter before making any write. Also inspect the linked assay artifact when the accepted unit carries an
`assayReceipt`. Preserve the receipt key; never create a replacement key to get around ambiguity.

## Record an operator disposition

Forge's retry error reports the expected `completion:v2:<sha256>` fingerprint. First establish that every effect
of that exact accepted unit is present. If an effect is missing, repair only the missing effect using the existing
authoritative service/DAO path, then verify the resulting rows. Do not authorize a whole-unit replay when any
counter or evidence effect may already have happened: repair/replan counters are increments and are not safe to
repeat.

After all effects are verified, acknowledge the accepted unit in a short write transaction. Include the operator
identity and the evidence reference in the message; use the fingerprint printed by the failed reconciliation.

```sql
begin;

update workflow_command_receipt
   set outcome = 'success',
       request_fingerprint = 'completion:v2:<sha256>',
       message = 'legacy completion reconciled; operator=<identity>; evidence=<reference>',
       updated_at = now()
 where command_id = 'forge.completion:<task UUID>'
   and outcome = 'quarantined'
returning command_id, outcome, request_fingerprint, message, updated_at;

-- Commit only after verifying the returned row is the intended receipt.
commit;
```

If the inspection proves that no effect from the old pending unit occurred, an operator may instead explicitly
authorize one atomic replay. Set the same expected fingerprint and a reason in `message`, then let the normal
completion reconciler run the unit. Forge consumes `replay_authorized` only when its current fingerprint matches;
the authorization and all effects then commit atomically.

```sql
begin;

update workflow_command_receipt
   set outcome = 'replay_authorized',
       request_fingerprint = 'completion:v2:<sha256>',
       message = 'operator authorized replay after proving no prior effects; operator=<identity>; evidence=<reference>',
       updated_at = now()
 where command_id = 'forge.completion:<task UUID>'
   and outcome = 'quarantined'
returning command_id, outcome, request_fingerprint, message, updated_at;

-- Commit only after verifying the returned row is the intended receipt.
commit;
```

If effects cannot be attributed conclusively, leave the receipt quarantined and escalate it. Never mark it successful
or authorize replay on age, story identity, or a log message alone.

import type { QueryExecutor } from '../db/query-executor'
import { QA_SOURCE_SYSTEM } from './qa-golden'

// ---------------------------------------------------------------------------
// QA-FIXTURE RESET — bounded, atomic, FK-safe Golden fixture teardown.
//
// Ordering contract against the canonical workflow schema
// (workflow_engine/scripts/schema.sql):
//   - tokens.process_instance_id   -> process_instances(id) ON DELETE CASCADE
//   - tasks.process_instance_id    -> process_instances(id) ON DELETE CASCADE
//   - tasks.token_id               -> tokens(id)          [NO cascade]
//   - jobs.process_instance_id     -> process_instances(id) ON DELETE CASCADE
//   - jobs.token_id                -> tokens(id)          [NO cascade]
//   - process_events.process_instance_id -> process_instances(id) [NO FK]
//
// Therefore we NEVER manually delete `tokens` (that would 23503 against
// tasks/jobs still referencing them). We delete the non-cascading evidence
// tables first (workflow_execution_trace_event, process_events), then delete
// process_instances and let the canonical cascades remove tokens/tasks/jobs.
// ---------------------------------------------------------------------------

export type GoldenResetTarget = {
  dealId: string
  propertyId: string
}

/**
 * Delete the entire bounded Golden QA fixture inside the supplied transaction
 * executor (`tx`). The caller wraps this in an interactive transaction so the
 * whole reset is atomic — if any delete fails the fixture is never half-reset.
 * Only Golden-owned rows are touched; unrelated DEV data is never modified.
 */
export async function resetGoldenData(
  tx: QueryExecutor,
  found: GoldenResetTarget,
): Promise<void> {
  // 1. All process instances belonging to the Golden deal.
  const instanceRows = (await tx`
    select id from process_instances
    where subject_type = 'deal' and subject_id = ${found.dealId}
  `) as Array<{ id: unknown }>

  // 2. Evidence tables that do NOT cascade from process_instances (fixture-owned).
  for (const row of instanceRows) {
    const id = String(row.id)
    await tx`delete from workflow_execution_trace_event where workflow_instance_id = ${id}`
    await tx`delete from process_events where process_instance_id = ${id}`
  }

  // 3. process_instances — the canonical ON DELETE CASCADE removes tokens/tasks/jobs.
  await tx`
    delete from process_instances
    where subject_type = 'deal' and subject_id = ${found.dealId}
  `

  // 4. The deal + its participant rows (deal_participant is canonical read state).
  await tx`delete from deal_participant where deal_id = ${found.dealId}`
  await tx`delete from deal where id = ${found.dealId}`

  // 5. Golden-owned persons by deterministic QA identity (never general DEV people).
  const qaPeople = (await tx`
    select distinct pi.person_id as id
    from person_identity pi
    where pi.source_system = ${QA_SOURCE_SYSTEM}
      and (pi.identity_value like 'qa-maria-%' or pi.identity_value like 'qa-juan-%')
  `) as Array<{ id: unknown }>
  for (const row of qaPeople) {
    await tx`delete from person where id = ${String(row.id)}`
  }

  // 6. The QA property.
  await tx`delete from property where id = ${found.propertyId}`
}

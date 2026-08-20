import { WorkflowEngine } from '../workflow_engine/lib/workflow/engine'
import { createApplicationPort } from './application-port'
import { engineConfigured, engineSql } from './engine-client'
import { getDealWorkflowFacts } from './facts'
import {
  TRANSACTION_CLOSE_V1_KEY,
  TRANSACTION_CLOSE_V1_VERSION,
} from './definitions/transaction-close-v1'
import { startWorkflowCore } from './start-core'

// ---------------------------------------------------------------------------
// Workflow start boundary — the application-side operation that turns a real
// accepted offer into a running transaction-close-v1 instance.
//
// The CulebraLuxe DB (canonical) and the workflow engine DB (orchestration)
// are separate databases, so "accept offer" and "start workflow" cannot be one
// ACID transaction. Atomicity is NOT faked:
//
//   acceptOffer (app DB)   ──commit──►  startTransactionCloseWorkflow (engine DB)
//                     ^                                 │
//                     └──── recovery window ────────────┘
//
// The recoverable handoff is `reconcileTransactionWorkflows()`: it finds deals
// with an accepted offer that have no active workflow instance and starts them
// idempotently. The engine's `process_instances_definition_subject_active_unique`
// partial unique index (migration 002) makes concurrent duplicate starts of the
// SAME definition fail deterministically, while still allowing a different
// workflow definition to be active for the same deal.
// ---------------------------------------------------------------------------

export async function findActiveInstance(
  dealId: string,
): Promise<string | null> {
  if (!engineConfigured()) return null
  const rows = await engineSql()`
    select pi.id
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'deal'
      and pi.subject_id = ${dealId}
      and pi.status = 'active'
      and pd.key = ${TRANSACTION_CLOSE_V1_KEY}
    limit 1
  `
  return (rows[0]?.id as string | undefined) ?? null
}

export async function startTransactionCloseWorkflow(
  dealId: string,
): Promise<{ instanceId: string; started: boolean }> {
  return startWorkflowCore(dealId, {
    findActive: findActiveInstance,
    readFacts: async (id) => {
      const facts = await getDealWorkflowFacts(id)
      return facts ? { financingApplicable: facts.financingApplicable } : null
    },
    start: async (id, financingApplicable) => {
      if (!engineConfigured()) {
        throw new Error('Workflow engine database is not configured.')
      }
      const engine = new WorkflowEngine(engineSql(), {
        app: createApplicationPort(),
      })
      const { processInstanceId } = await engine.startProcess({
        definitionKey: TRANSACTION_CLOSE_V1_KEY,
        version: TRANSACTION_CLOSE_V1_VERSION,
        startedBy: 'system',
        variables: { financingApplicable },
        subject: { subjectType: 'deal', subjectId: id },
      })
      return processInstanceId
    },
  })
}

export async function reconcileTransactionWorkflows(): Promise<string[]> {
  const { sql } = await import('../db/client')
  const dealRows = await sql`
    select distinct o.deal_id
    from offer o
    where o.status = 'accepted'
  `
  const started: string[] = []
  for (const row of dealRows as Array<{ deal_id: string }>) {
    const existing = await findActiveInstance(row.deal_id)
    if (!existing) {
      const { instanceId } = await startTransactionCloseWorkflow(row.deal_id)
      started.push(instanceId)
    }
  }
  return started
}

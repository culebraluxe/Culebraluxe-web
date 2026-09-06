import { WorkflowEngine } from '../workflow_engine/lib/workflow/engine'
import { createApplicationPort } from './application-port'
import { engineConfigured, engineSql } from './engine-client'
import { getDealWorkflowFacts } from './facts'
import { getContractWorkflowFacts } from './contract-facts'
import {
  RESIDENTIAL_TRANSACTION_KEY,
  RESIDENTIAL_TRANSACTION_VERSION,
} from './workflow-config'
import { startWorkflowCore } from './start-core'

// Legacy Deal starter is retained until the rest of the transaction surface is
// strangled. New P&S execution uses the Contract starter below.
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
      and pd.key = ${RESIDENTIAL_TRANSACTION_KEY}
    limit 1
  `
  return (rows[0]?.id as string | undefined) ?? null
}

export async function findActiveContractInstance(
  contractId: string,
): Promise<string | null> {
  if (!engineConfigured()) return null
  const rows = await engineSql()`
    select pi.id
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'contract'
      and pi.subject_id = ${contractId}
      and pi.status = 'active'
      and pd.key = ${RESIDENTIAL_TRANSACTION_KEY}
    limit 1
  `
  return (rows[0]?.id as string | undefined) ?? null
}

export async function startResidentialTransactionWorkflow(
  dealId: string,
): Promise<{ instanceId: string; started: boolean }> {
  return startWorkflowCore(dealId, {
    findActive: findActiveInstance,
    readFacts: async (id) => {
      const facts = await getDealWorkflowFacts(id)
      return facts ? (facts as unknown as Record<string, any>) : null
    },
    start: async (id, facts) => {
      if (!engineConfigured()) {
        throw new Error('Workflow engine database is not configured.')
      }
      const engine = new WorkflowEngine(engineSql(), {
        app: createApplicationPort(),
      })
      const { processInstanceId } = await engine.startProcess({
        definitionKey: RESIDENTIAL_TRANSACTION_KEY,
        version: RESIDENTIAL_TRANSACTION_VERSION,
        startedBy: 'system',
        variables: facts,
        subject: { subjectType: 'deal', subjectId: id },
      })
      return processInstanceId
    },
  })
}

/**
 * Canonical P&S workflow start seam. Contract is the subject and fact authority;
 * no Deal id is needed to create or correlate the workflow instance.
 */
export async function startResidentialContractWorkflow(
  contractId: string,
): Promise<{ instanceId: string; started: boolean }> {
  return startWorkflowCore(contractId, {
    findActive: findActiveContractInstance,
    readFacts: async (id) => {
      const facts = await getContractWorkflowFacts(id)
      return facts ? (facts as Record<string, any>) : null
    },
    start: async (id, facts) => {
      if (!engineConfigured()) {
        throw new Error('Workflow engine database is not configured.')
      }
      const engine = new WorkflowEngine(engineSql(), {
        app: createApplicationPort(),
      })
      const { processInstanceId } = await engine.startProcess({
        definitionKey: RESIDENTIAL_TRANSACTION_KEY,
        version: RESIDENTIAL_TRANSACTION_VERSION,
        startedBy: 'system',
        variables: facts,
        subject: { subjectType: 'contract', subjectId: id },
      })
      return processInstanceId
    },
  })
}

export async function reconcileResidentialTransactionWorkflows(): Promise<string[]> {
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
      const { instanceId } = await startResidentialTransactionWorkflow(row.deal_id)
      started.push(instanceId)
    }
  }
  return started
}

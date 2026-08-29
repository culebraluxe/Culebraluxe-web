// ---------------------------------------------------------------------------
// DEV-ONLY QA seed — durable "Flight Recorder Golden Purchase" transaction.
//
// Creates (or reuses) a persistent QA property, QA client persons, a QA deal,
// and a REAL residential workflow instance via the real workflow engine — then
// adds clearly-marked QA simulation trace evidence so the Flight Recorder shows
// a rich COMPLETED / CURRENT / FUTURE journey.
//
// DEV ONLY. Refuses to run outside DEV. Idempotent. --reset removes ONLY the
// golden fixture (by a deterministic marker), never general DEV data.
// ---------------------------------------------------------------------------
import { randomUUID } from 'node:crypto'

import { createProperty } from '../db/property-admin-writes'
import { createPersonWithIdentities } from '../db/person-identities'
import { createDeal } from '../db/deal-admin-writes'
import { recordTraceEvent } from '../db/workflow-trace'
import { sql } from '../db/client'
import { engineConfigured, engineSql } from '../workflow_app/engine-client'
import { createApplicationPort } from '../workflow_app/application-port'
import { WorkflowEngine } from '../workflow_engine/lib/workflow/engine'
import {
  RESIDENTIAL_TRANSACTION_KEY,
  RESIDENTIAL_TRANSACTION_VERSION,
} from '../workflow_app/workflow-config'
import { QA_GOLDEN_DEAL_MARKER as QA_MARKER } from '../workflow_app/flight-recorder-read'

// Deterministic marker used to identify the golden fixture for reset/re-seed.
const QA_PROPERTY_SLUG = 'qa-flight-recorder-golden-purchase'

function isProduction(): boolean {
  return (
    process.env.NODE_ENV === 'production' ||
    process.env.APP_ENV === 'production' ||
    process.env.VERCEL_ENV === 'production'
  )
}

type Golden = {
  dealId: string
  propertyId: string
  personId: string
  instanceId: string | null
}

async function findGolden(): Promise<Golden | null> {
  const dealRows = await sql`
    select d.id as deal_id, d.property_id, d.client_person_id
    from deal d
    where d.notes = ${QA_MARKER}
    limit 1
  `
  if (dealRows.length === 0) return null
  const r = dealRows[0] as { deal_id: unknown; property_id: unknown; client_person_id: unknown }
  const instRows = await sql`
    select pi.id
    from process_instances pi
    where pi.subject_type = 'deal' and pi.subject_id = ${String(r.deal_id)}
    order by pi.created_at asc
    limit 1
  `
  return {
    dealId: String(r.deal_id),
    propertyId: String(r.property_id),
    personId: String(r.client_person_id),
    instanceId: instRows[0] ? String((instRows[0] as { id: unknown }).id) : null,
  }
}

async function resetGolden(): Promise<void> {
  const found = await findGolden()
  if (!found) {
    console.log('[flight-recorder-qa] nothing to reset (no golden fixture).')
    return
  }
  const instanceRows = await sql`
    select id from process_instances
    where subject_type = 'deal' and subject_id = ${found.dealId}
  `
  for (const row of instanceRows as Array<{ id: unknown }>) {
    const id = String(row.id)
    await sql`delete from workflow_execution_trace_event where workflow_instance_id = ${id}`
    await sql`delete from tokens where process_instance_id = ${id}`
    await sql`delete from process_instances where id = ${id}`
  }
  await sql`delete from deal_participant where deal_id = ${found.dealId}`
  await sql`delete from deal where id = ${found.dealId}`
  await sql`delete from person where id = ${found.personId}`
  await sql`delete from property where id = ${found.propertyId}`
  console.log('[flight-recorder-qa] reset complete (golden fixture only).')
}
async function startGoldenWorkflow(dealId: string): Promise<string | null> {
  if (!engineConfigured()) return null
  try {
    const engine = new WorkflowEngine(engineSql(), { app: createApplicationPort() })
    const { processInstanceId } = await engine.startProcess({
      definitionKey: RESIDENTIAL_TRANSACTION_KEY,
      version: RESIDENTIAL_TRANSACTION_VERSION,
      startedBy: 'flight-recorder-qa',
      variables: {},
      subject: { subjectType: 'deal', subjectId: dealId },
    })
    return processInstanceId
  } catch (err) {
    console.error('[flight-recorder-qa] workflow start failed:', err)
    return null
  }
}

async function recordQaEvidence(instanceId: string): Promise<void> {
  const now = new Date().toISOString()
  const mark = (m: Record<string, unknown>) => ({ qa_simulation: true, ...m })
  const rows: Array<{
    eventType: string
    system: string
    summary: string
    metadata: Record<string, unknown>
  }> = [
    {
      eventType: 'COMMAND_RECEIVED',
      system: 'command',
      summary: 'QA Create Deal command received',
      metadata: mark({ command: 'deal.create' }),
    },
    {
      eventType: 'DOCUMENT_CREATED',
      system: 'document',
      summary: 'QA Purchase Contract generated',
      metadata: mark({ qa: true }),
    },
    {
      eventType: 'SIGNATURE_REQUEST_CREATED',
      system: 'signature',
      summary: 'QA Signature request created (simulated)',
      metadata: mark({ provider: 'qa-simulation' }),
    },
    {
      eventType: 'SIGNATURE_SENT',
      system: 'signature',
      summary: 'QA Signature request sent (simulated provider)',
      metadata: mark({ provider: 'qa-simulation' }),
    },
  ]
  for (const row of rows) {
    await recordTraceEvent({
      eventType: row.eventType,
      system: row.system,
      occurredAt: now,
      workflowInstanceId: instanceId,
      summary: row.summary,
      metadata: row.metadata,
    })
  }
}

function printResult(r: Golden): void {
  console.log(`
Flight Recorder QA fixture ready

  Deal:              ${r.dealId}
  Property:          QA — 123 Ocean View Drive
  Client:            QA Maria Rodriguez
  Workflow Instance: ${r.instanceId ?? '(none — workflow start failed)'}
  Definition:        ${RESIDENTIAL_TRANSACTION_KEY} v${RESIDENTIAL_TRANSACTION_VERSION}

  Open:
    /portal/tech/flight-recorder/${r.instanceId ?? '<instanceId>'}

  Reset:
    pnpm flight-recorder:qa-reset
`)
}

async function seed(): Promise<void> {
  const existing = await findGolden()
  if (existing && existing.instanceId) {
    printResult(existing)
    return
  }

  const prop = await createProperty({
    name: 'QA — 123 Ocean View Drive',
    slug: QA_PROPERTY_SLUG,
    status: 'active',
    featured: false,
    propertyType: 'single_family',
    listPrice: 850000,
    location: '123 Ocean View Drive',
    city: 'Culebra',
    stateOrProvince: 'PR',
    neighborhood: 'Culebra',
    bedrooms: 3,
    bathrooms: 2,
    squareFeet: 1800,
  })

  const mariaId = randomUUID()
  const juanId = randomUUID()
  await createPersonWithIdentities({
    personId: mariaId,
    displayName: 'QA Maria Rodriguez',
    role: 'buyer',
    identities: [
      { kind: 'external', normalizedValue: `qa-maria-${mariaId}`, sourceSystem: 'flight-recorder-qa', isPrimary: true },
    ],
  })
  await createPersonWithIdentities({
    personId: juanId,
    displayName: 'QA Juan Rodriguez',
    role: 'buyer',
    identities: [
      { kind: 'external', normalizedValue: `qa-juan-${juanId}`, sourceSystem: 'flight-recorder-qa', isPrimary: true },
    ],
  })

  const deal = await createDeal({
    propertyId: prop.id,
    clientPersonId: mariaId,
    notes: QA_MARKER,
  })

  const instanceId = await startGoldenWorkflow(deal.id)
  if (instanceId) {
    await recordQaEvidence(instanceId)
  }

  printResult({ dealId: deal.id, propertyId: prop.id, personId: mariaId, instanceId })
}

const reset = process.argv.includes('--reset')
;(async () => {
  if (isProduction()) {
    console.error('[flight-recorder-qa] REFUSING: this seeds QA data and must never run in production.')
    process.exit(1)
  }
  if (reset) await resetGolden()
  await seed()
})().catch((err) => {
  console.error('[flight-recorder-qa] failed:', err)
  process.exit(1)
})


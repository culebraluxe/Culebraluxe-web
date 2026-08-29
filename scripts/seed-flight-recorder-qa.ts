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
import {
  buildGoldenEventSpecs,
  QA_SOURCE_SYSTEM,
  QA_FIXTURE_VERSION,
  type QaContext,
} from '../lib/qa-golden'

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

async function ensureDealParticipants(
  dealId: string,
  mariaId: string,
  juanId: string,
): Promise<void> {
  for (const personId of [mariaId, juanId]) {
    const exists = await sql`
      select 1 from deal_participant where deal_id = ${dealId} and person_id = ${personId} limit 1
    `
    if (exists.length === 0) {
      await sql`
        insert into deal_participant (deal_id, person_id, role, active)
        values (${dealId}, ${personId}, 'client', true)
      `
    }
  }
}

async function findQaPersonByPrefix(prefix: string): Promise<string | null> {
  const rows = await sql`
    select pi.person_id
    from person_identity pi
    where pi.source_system = ${QA_SOURCE_SYSTEM} and pi.identity_value like ${`${prefix}-%`}
    limit 1
  `
  return rows[0] ? String((rows[0] as { person_id: unknown }).person_id) : null
}

async function loadWorkflowMeta(instanceId: string): Promise<{
  definitionKey: string
  definitionVersion: number
}> {
  const rows = await sql`
    select pd.key as def_key, pd.version as def_version
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.id = ${instanceId}
    limit 1
  `
  const r = rows[0] as { def_key?: unknown; def_version?: unknown } | undefined
  return {
    definitionKey: r?.def_key == null ? RESIDENTIAL_TRANSACTION_KEY : String(r.def_key),
    definitionVersion: r?.def_version == null ? RESIDENTIAL_TRANSACTION_VERSION : Number(r.def_version),
  }
}

/**
 * Rebuild the deterministic 18-event Golden QA narrative for the instance.
 * Deletes any prior QA-marked / narrative events for this instance, then inserts
 * the 18 events with real timing offsets and causation resolved to the ACTUAL
 * persisted event ids (via the deterministic source_system/source_event_id).
 */
async function rebuildGoldenEvidence(instanceId: string, ctx: QaContext): Promise<void> {
  await sql`
    delete from workflow_execution_trace_event
    where workflow_instance_id = ${instanceId}
      and (source_system = ${QA_SOURCE_SYSTEM} or metadata->>'qa_simulation' = 'true')
  `

  const traceStart = new Date()
  const ids = new Map<string, string>()
  for (const spec of buildGoldenEventSpecs()) {
    const causeId = spec.causeSourceEventId ? (ids.get(spec.causeSourceEventId) ?? null) : null
    await recordTraceEvent({
      eventType: spec.eventType,
      system: spec.system,
      occurredAt: new Date(traceStart.getTime() + spec.offsetMs).toISOString(),
      workflowInstanceId: instanceId,
      summary: spec.summary,
      causationId: causeId,
      metadata: { ...spec.metadata(ctx), qa_fixture_version: QA_FIXTURE_VERSION },
      sourceSystem: QA_SOURCE_SYSTEM,
      sourceEventId: spec.sourceEventId,
    })
    const row = await sql`
      select id from workflow_execution_trace_event
      where source_system = ${QA_SOURCE_SYSTEM} and source_event_id = ${spec.sourceEventId}
      limit 1
    `
    ids.set(spec.sourceEventId, String((row[0] as { id: unknown }).id))
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
    // Reuse the durable golden fixture; ensure both QA clients participate.
    const mariaId = existing.personId
    const juanId = (await findQaPersonByPrefix('qa-juan')) ?? mariaId
    await ensureDealParticipants(existing.dealId, mariaId, juanId)
    const meta = await loadWorkflowMeta(existing.instanceId)
    const ctx: QaContext = {
      dealId: existing.dealId,
      propertyId: existing.propertyId,
      propertyName: 'QA — 123 Ocean View Drive',
      mariaId,
      juanId,
      mariaName: 'QA Maria Rodriguez',
      juanName: 'QA Juan Rodriguez',
      workflowInstanceId: existing.instanceId,
      workflowDefinitionKey: meta.definitionKey,
      workflowDefinitionVersion: meta.definitionVersion,
    }
    await rebuildGoldenEvidence(existing.instanceId, ctx)
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
      { kind: 'external', normalizedValue: `qa-maria-${mariaId}`, sourceSystem: QA_SOURCE_SYSTEM, isPrimary: true },
    ],
  })
  await createPersonWithIdentities({
    personId: juanId,
    displayName: 'QA Juan Rodriguez',
    role: 'buyer',
    identities: [
      { kind: 'external', normalizedValue: `qa-juan-${juanId}`, sourceSystem: QA_SOURCE_SYSTEM, isPrimary: true },
    ],
  })

  const deal = await createDeal({
    propertyId: prop.id,
    clientPersonId: mariaId,
    notes: QA_MARKER,
  })
  await ensureDealParticipants(deal.id, mariaId, juanId)

  const instanceId = await startGoldenWorkflow(deal.id)
  if (instanceId) {
    const meta = await loadWorkflowMeta(instanceId)
    const ctx: QaContext = {
      dealId: deal.id,
      propertyId: prop.id,
      propertyName: 'QA — 123 Ocean View Drive',
      mariaId,
      juanId,
      mariaName: 'QA Maria Rodriguez',
      juanName: 'QA Juan Rodriguez',
      workflowInstanceId: instanceId,
      workflowDefinitionKey: meta.definitionKey,
      workflowDefinitionVersion: meta.definitionVersion,
    }
    await rebuildGoldenEvidence(instanceId, ctx)
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


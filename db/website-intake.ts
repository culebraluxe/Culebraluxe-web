import { sql } from './client'

import type {
  CanonicalWebsiteIntakeInput,
  WebsiteIntakePayload,
  WebsiteIntakeReceipt,
  WebsiteIntakeStatus,
} from '../lib/website-intake-types'
import type {
  QueryExecutor,
  QueryRow,
  TransactionExecutor,
} from './query-executor'

type ReceiptRow = {
  id: string
  request_type: WebsiteIntakeReceipt['requestType']
  property_id: string
  display_name: string
  email: string
  message: string | null
  status: WebsiteIntakeStatus
  processing_started_at: string | null
  interaction_id: string | null
  created_at: string
  updated_at: string
}

const runNeonTransaction: TransactionExecutor = async (buildQueries) =>
  (await sql.transaction((transactionSql) =>
    buildQueries(transactionSql as unknown as QueryExecutor) as never,
  )) as QueryRow[][]

function mapReceipt(row: ReceiptRow): WebsiteIntakeReceipt {
  return {
    submissionId: row.id,
    requestType: row.request_type,
    propertyId: row.property_id,
    displayName: row.display_name,
    email: row.email,
    message: row.message ?? undefined,
    status: row.status,
    processingStartedAt: row.processing_started_at ?? undefined,
    interactionId: row.interaction_id ?? undefined,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

export async function insertOrReadWebsiteIntakeReceipt(
  input: WebsiteIntakePayload,
  execute: QueryExecutor = sql,
): Promise<{ receipt: WebsiteIntakeReceipt; created: boolean }> {
  const inserted = await execute`
    insert into website_intake_submission (
      id, request_type, property_id, display_name, email, message
    ) values (
      ${input.submissionId}, ${input.requestType}, ${input.propertyId},
      ${input.displayName}, ${input.email}, ${input.message ?? null}
    )
    on conflict (id) do nothing
    returning id, request_type, property_id, display_name, email, message,
      status, processing_started_at, interaction_id, created_at, updated_at
  `
  const created = inserted[0] as ReceiptRow | undefined
  if (created) return { receipt: mapReceipt(created), created: true }

  const existing = await execute`
    select id, request_type, property_id, display_name, email, message,
      status, processing_started_at, interaction_id, created_at, updated_at
    from website_intake_submission
    where id = ${input.submissionId}
    limit 1
  `
  const row = existing[0] as ReceiptRow | undefined
  if (!row) throw new Error('Website intake receipt could not be resolved.')
  return { receipt: mapReceipt(row), created: false }
}

export async function claimWebsiteIntakeReceipt(
  submissionId: string,
  options: { trustedResolutionRetry?: boolean } = {},
  execute: QueryExecutor = sql,
): Promise<WebsiteIntakeReceipt | null> {
  const trustedRetry = options.trustedResolutionRetry === true
  const rows = await execute`
    update website_intake_submission
    set status = 'processing',
        processing_started_at = now(),
        updated_at = now()
    where id = ${submissionId}
      and (
        status = 'received'
        or (status = 'processing'
          and processing_started_at <= now() - interval '15 minutes')
        or (${trustedRetry} and status = 'resolution_required')
      )
    returning id, request_type, property_id, display_name, email, message,
      status, processing_started_at, interaction_id, created_at, updated_at
  `
  const row = rows[0] as ReceiptRow | undefined
  return row ? mapReceipt(row) : null
}

const allowedTransitions = new Set([
  'processing:completed',
  'processing:resolution_required',
  'processing:rejected',
  'processing:received',
])

export async function transitionWebsiteIntakeReceipt(
  input: {
    submissionId: string
    claimToken: string
    from: WebsiteIntakeStatus
    to: Exclude<WebsiteIntakeStatus, 'processing'>
    interactionId?: string
  },
  execute: QueryExecutor = sql,
) {
  if (!allowedTransitions.has(`${input.from}:${input.to}`)) {
    throw new Error('Website intake receipt transition is not allowed.')
  }
  if ((input.to === 'completed') !== Boolean(input.interactionId)) {
    throw new Error('Only a completed receipt may have an interaction ID.')
  }

  const rows = await execute`
    update website_intake_submission
    set status = ${input.to},
        processing_started_at = null,
        interaction_id = ${input.interactionId ?? null},
        updated_at = now()
    where id = ${input.submissionId}
      and status = ${input.from}
      and processing_started_at = ${input.claimToken}
    returning id
  `
  return rows.length === 1
}

export async function persistCanonicalWebsiteIntake(
  input: CanonicalWebsiteIntakeInput,
  executeTransaction: TransactionExecutor = runNeonTransaction,
  executeLookup: QueryExecutor = sql,
): Promise<{ interactionId: string; created: boolean }> {
  const eventType =
    input.requestType === 'private_viewing'
      ? 'private_viewing_requested'
      : 'property_inquiry_submitted'
  const title =
    input.requestType === 'private_viewing'
      ? 'Private viewing request'
      : 'Property information request'
  const taskTitle =
    input.requestType === 'private_viewing'
      ? `Follow up on private viewing request from ${input.displayName}`
      : `Follow up on property inquiry from ${input.displayName}`

  const results = await executeTransaction((execute) => [
    execute`
      insert into interaction (
        id, person_id, property_id, channel, event_type, direction,
        occurred_at, title, summary, source_system, source_external_id,
        source_metadata
      ) values (
        ${input.interactionId}, ${input.personId}, ${input.propertyId},
        'website', ${eventType}, 'inbound', ${input.occurredAt}, ${title},
        ${input.message ?? null}, 'website', ${input.submissionId},
        ${JSON.stringify({ requestType: input.requestType })}::jsonb
      )
      on conflict (source_system, source_external_id)
        where source_system is not null and source_external_id is not null
      do nothing
      returning id
    `,
    execute`
      insert into property_interest (person_id, property_id, status)
      select ${input.personId}, ${input.propertyId}, 'interested'
      where exists (
        select 1 from interaction where id = ${input.interactionId}
      )
      on conflict (person_id, property_id) do nothing
      returning id
    `,
    execute`
      insert into task (
        title, detail, person_id, property_id, source_interaction_id,
        task_kind, priority
      )
      select ${taskTitle}, ${input.message ?? null}, ${input.personId},
        ${input.propertyId}, ${input.interactionId}, 'human', 0
      where exists (
        select 1 from interaction where id = ${input.interactionId}
      )
      returning id
    `,
  ])

  const created = results[0]?.length === 1
  if (created) return { interactionId: input.interactionId, created: true }

  const lookup = await executeLookup`
    select id
    from interaction
    where source_system = 'website'
      and source_external_id = ${input.submissionId}
    limit 1
  `
  const existing = lookup[0] as { id: string } | undefined
  if (!existing) throw new Error('Canonical website interaction could not be resolved.')
  return { interactionId: existing.id, created: false }
}

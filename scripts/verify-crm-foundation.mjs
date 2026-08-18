import assert from 'node:assert/strict'

const { createInteraction, getInteractionById } = await import(
  '../db/interactions.ts'
)
const { createTask, getTaskById } = await import('../db/tasks.ts')

function queuedExecutor(responses) {
  const calls = []
  const execute = async (strings, ...parameters) => {
    calls.push({ sql: strings.join('?'), parameters })
    return responses.shift() ?? []
  }

  return { calls, execute }
}

const baseInteractionRow = {
  id: '10000000-0000-4000-8000-000000000001',
  person_id: '20000000-0000-4000-8000-000000000001',
  property_id: null,
  deal_id: null,
  channel: 'email',
  event_type: 'email',
  direction: 'inbound',
  occurred_at: '2026-08-18T12:00:00.000Z',
  title: 'Property inquiry',
  summary: null,
  duration_seconds: null,
  source_system: 'website',
  source_external_id: 'lead-123',
  source_metadata: {},
  created_at: '2026-08-18T12:00:01.000Z',
}

{
  const fake = queuedExecutor([[baseInteractionRow]])
  const interaction = await getInteractionById(
    baseInteractionRow.id,
    fake.execute,
  )

  assert.equal(interaction?.eventType, 'email')
  assert.deepEqual(interaction?.sourceMetadata, {})
}

{
  const createdRow = {
    ...baseInteractionRow,
    event_type: 'lead_submitted',
    channel: 'website',
    source_metadata: { campaign: 'seller-demo' },
  }
  const fake = queuedExecutor([[createdRow]])
  const result = await createInteraction(
    {
      personId: createdRow.person_id,
      channel: 'website',
      eventType: 'lead_submitted',
      occurredAt: createdRow.occurred_at,
      sourceSystem: createdRow.source_system,
      sourceExternalId: createdRow.source_external_id,
      sourceMetadata: createdRow.source_metadata,
    },
    fake.execute,
  )

  assert.equal(result.created, true)
  assert.equal(result.interaction.eventType, 'lead_submitted')
  assert.deepEqual(result.interaction.sourceMetadata, {
    campaign: 'seller-demo',
  })
  assert.match(fake.calls[0].sql, /on conflict \(source_system, source_external_id\)/i)
}

{
  const fake = queuedExecutor([[], [baseInteractionRow]])
  const result = await createInteraction(
    {
      personId: baseInteractionRow.person_id,
      channel: 'email',
      eventType: 'email_received',
      occurredAt: baseInteractionRow.occurred_at,
      sourceSystem: 'website',
      sourceExternalId: 'lead-123',
    },
    fake.execute,
  )

  assert.equal(result.created, false)
  assert.equal(result.interaction.id, baseInteractionRow.id)
  assert.equal(fake.calls.length, 2)
}

{
  const fake = queuedExecutor([])
  await assert.rejects(
    createInteraction(
      {
        personId: baseInteractionRow.person_id,
        channel: 'manual',
        eventType: 'note',
        occurredAt: baseInteractionRow.occurred_at,
        sourceSystem: 'manual',
      },
      fake.execute,
    ),
    /provided together/,
  )
  assert.equal(fake.calls.length, 0)
}

const baseTaskRow = {
  id: '30000000-0000-4000-8000-000000000001',
  title: 'Follow up',
  detail: null,
  person_id: baseInteractionRow.person_id,
  property_id: null,
  deal_id: null,
  source_interaction_id: baseInteractionRow.id,
  assigned_user_id: null,
  due_at: null,
  task_kind: 'system',
  priority: 2,
  status: 'open',
  completed_at: null,
  created_at: '2026-08-18T12:01:00.000Z',
  updated_at: '2026-08-18T12:01:00.000Z',
}

{
  const fake = queuedExecutor([[baseTaskRow]])
  const task = await getTaskById(baseTaskRow.id, fake.execute)
  assert.equal(task?.taskKind, 'system')
  assert.equal(task?.priority, 2)
  assert.equal(task?.sourceInteractionId, baseInteractionRow.id)
}

{
  const fake = queuedExecutor([[baseTaskRow]])
  const task = await createTask(
    {
      title: 'Follow up',
      personId: baseTaskRow.person_id,
      sourceInteractionId: baseInteractionRow.id,
      taskKind: 'system',
      priority: 2,
    },
    fake.execute,
  )

  assert.equal(task.taskKind, 'system')
  assert.equal(task.priority, 2)
  assert.equal(task.sourceInteractionId, baseInteractionRow.id)
}

{
  const fake = queuedExecutor([])
  await assert.rejects(
    createTask({ title: 'Missing context' }, fake.execute),
    /requires person, property, or deal context/,
  )
  assert.equal(fake.calls.length, 0)
}

console.log('CRM foundation verification passed without database access.')

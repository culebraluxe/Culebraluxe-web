import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const {
  adaptWebsiteIntake,
  normalizeWebsiteIntake,
  parseWebsiteIntakeFormData,
  processWebsiteIntake,
  WebsiteIntakeTransientError,
  websitePayloadsEqual,
} = await import('../lib/website-intake.ts')
const {
  claimWebsiteIntakeReceipt,
  persistCanonicalWebsiteIntake,
  transitionWebsiteIntakeReceipt,
} = await import('../db/website-intake.ts')

const SUBMISSION = '40000000-0000-4000-8000-000000000010'
const PROPERTY = '40000000-0000-4000-8000-000000000001'
const PERSON = '10000000-0000-4000-8000-000000000001'
const INTERACTION = '50000000-0000-4000-8000-000000000001'
const NOW = '2026-08-18T16:00:00.000Z'

function payload(overrides = {}) {
  return normalizeWebsiteIntake({
    submissionId: SUBMISSION,
    requestType: 'private_viewing',
    propertyId: PROPERTY,
    name: '  María   Rivera ',
    email: ' BUYER@Example.com ',
    message: ' I would like to visit. ',
    ...overrides,
  })
}

function receipt(input, status = 'received', extras = {}) {
  return {
    ...input,
    status,
    createdAt: NOW,
    updatedAt: NOW,
    ...extras,
  }
}

function fakeSystem(options = {}) {
  const receipts = new Map()
  const identities = new Map(options.identities ?? [])
  const interactions = new Map()
  const calls = {
    neon: 0,
    claims: 0,
    canonical: 0,
    creates: 0,
    transitions: [],
  }
  let claimSequence = 0

  const findDuplicate = async (system, externalId) =>
    interactions.get(`${system}:${externalId}`) ?? null

  const crm = {
    findInteractionBySourceIdentity: findDuplicate,
    async personExists(personId) {
      return personId === PERSON
    },
    async findIdentityMatch(hint) {
      const owner = identities.get(`${hint.kind}:${hint.normalizedValue}`)
      return owner
        ? {
            identityId: `identity-${owner}`,
            personId: owner,
            kind: hint.kind,
            normalizedValue: hint.normalizedValue,
          }
        : null
    },
    async findIdentityOwnership(hint) {
      const owner = identities.get(`${hint.kind}:${hint.normalizedValue}`)
      return owner
        ? {
            identityId: `identity-${owner}`,
            personId: owner,
            kind: hint.kind,
            normalizedValue: hint.normalizedValue,
            archived: false,
          }
        : null
    },
    async createPersonWithIdentities(input) {
      calls.creates += 1
      for (const identity of input.identities) {
        identities.set(`${identity.kind}:${identity.normalizedValue}`, input.personId)
      }
    },
    async findPropertyById(id) {
      return id === PROPERTY ? { id } : null
    },
    async findPropertyBySlug() {
      return null
    },
    async findDealById() {
      return null
    },
  }

  const repositories = {
    crm,
    async findActiveProperty(id) {
      return id === PROPERTY ? { id } : null
    },
    async insertOrReadReceipt(input) {
      const existing = receipts.get(input.submissionId)
      if (existing) return { receipt: existing, created: false }
      const created = receipt(input)
      receipts.set(input.submissionId, created)
      return { receipt: created, created: true }
    },
    async claimReceipt(id, claimOptions = {}) {
      const current = receipts.get(id)
      if (!current) return null
      const eligible =
        current.status === 'received' ||
        (current.status === 'resolution_required' &&
          claimOptions.trustedResolutionRetry) ||
        (current.status === 'processing' && options.staleProcessing)
      if (!eligible) return null
      calls.claims += 1
      claimSequence += 1
      const claimed = receipt(current, 'processing', {
        processingStartedAt: new Date(
          new Date(NOW).getTime() + claimSequence * 1000,
        ).toISOString(),
      })
      receipts.set(id, claimed)
      return claimed
    },
    async transitionReceipt(input) {
      calls.transitions.push(input)
      const current = receipts.get(input.submissionId)
      if (
        !current ||
        current.status !== input.from ||
        current.processingStartedAt !== input.claimToken
      ) return false
      const next = receipt(current, input.to, {
        processingStartedAt: undefined,
        interactionId: input.interactionId,
      })
      receipts.set(input.submissionId, next)
      return true
    },
    async persistCanonical(input) {
      calls.canonical += 1
      if (options.transientFailure) {
        throw new WebsiteIntakeTransientError('transient')
      }
      const key = `website:${input.submissionId}`
      const existing = interactions.get(key)
      if (existing) return { interactionId: existing.id, created: false }
      const interaction = {
        id: INTERACTION,
        personId: input.personId,
        propertyId: input.propertyId,
        sourceSystem: 'website',
        sourceExternalId: input.submissionId,
      }
      interactions.set(key, interaction)
      return { interactionId: interaction.id, created: true }
    },
  }

  return { calls, receipts, interactions, repositories }
}

const migration = await readFile(
  new URL('../db/migrations/006_website_intake_submission.sql', import.meta.url),
  'utf8',
)
assert.match(migration, /references interaction\(id\) on delete restrict/i)
assert.match(migration, /website_intake_interaction_state/)
assert.match(migration, /website_intake_processing_state/)
assert.doesNotMatch(migration, /person_id/i)
assert.doesNotMatch(migration, /jsonb|metadata|attachment/i)

const normalized = payload()
assert.equal(normalized.displayName, 'María Rivera')
assert.equal(normalized.email, 'buyer@example.com')
assert.equal(normalized.message, 'I would like to visit.')
assert.throws(() => payload({ submissionId: 'bad' }), /UUID/)
assert.throws(() => payload({ requestType: 'seller' }), /invalid/)
assert.throws(() => payload({ message: 'x'.repeat(4001) }), /too long/)
assert.equal(websitePayloadsEqual(normalized, { ...normalized }), true)
assert.equal(websitePayloadsEqual(normalized, { ...normalized, message: 'changed' }), false)

const adapted = adaptWebsiteIntake(normalized, NOW)
assert.equal(adapted.event.actor.identityHints[0].evidence, 'user_supplied')
assert.equal(adapted.event.context.propertyId, PROPERTY)
assert.equal(adapted.event.intentHints.requestedAction, 'private_viewing')
assert.equal('personId' in adapted.payload, false)

// Form parsing recognizes the honeypot before validation and ignores browser
// attempts to supply trusted identity or receipt/canonical state.
{
  const honey = new FormData()
  honey.set('company', 'robot')
  assert.deepEqual(parseWebsiteIntakeFormData(honey), { honeypot: true })

  const form = new FormData()
  form.set('submissionId', SUBMISSION)
  form.set('requestType', 'private_viewing')
  form.set('propertyId', PROPERTY)
  form.set('name', 'María Rivera')
  form.set('email', 'buyer@example.com')
  form.set('personId', PERSON)
  form.set('emailEvidence', 'authenticated')
  form.set('status', 'completed')
  form.set('interactionId', INTERACTION)
  const parsed = parseWebsiteIntakeFormData(form)
  assert.equal(parsed.honeypot, false)
  assert.equal('personId' in parsed.payload, false)
  assert.equal('emailEvidence' in parsed.payload, false)
  assert.equal('status' in parsed.payload, false)
  assert.equal('interactionId' in parsed.payload, false)
}

// Exact existing identity completes; one public replay performs no second claim/write.
{
  const fake = fakeSystem({
    identities: [['email:buyer@example.com', PERSON]],
  })
  const result = await processWebsiteIntake(normalized, {
    repositories: fake.repositories,
    createId: () => INTERACTION,
    now: () => new Date(NOW),
  })
  assert.deepEqual(result, { accepted: true, status: 'accepted' })
  assert.equal(fake.receipts.get(SUBMISSION).status, 'completed')
  assert.equal(fake.receipts.get(SUBMISSION).interactionId, INTERACTION)
  assert.equal(fake.calls.canonical, 1)
  await processWebsiteIntake(normalized, {
    repositories: fake.repositories,
    createId: () => INTERACTION,
  })
  assert.equal(fake.calls.claims, 1)
  assert.equal(fake.calls.canonical, 1)
}

// Unknown public identity is retained for resolution with zero canonical writes.
{
  const fake = fakeSystem()
  await processWebsiteIntake(normalized, { repositories: fake.repositories })
  assert.equal(fake.receipts.get(SUBMISSION).status, 'resolution_required')
  assert.equal(fake.calls.canonical, 0)
  assert.equal(fake.calls.creates, 0)
  await processWebsiteIntake(normalized, { repositories: fake.repositories })
  assert.equal(fake.calls.claims, 1)
}

// A trusted retry reuses the receipt UUID and CRM-03 may safely create.
{
  const fake = fakeSystem()
  fake.receipts.set(SUBMISSION, receipt(normalized, 'resolution_required'))
  await processWebsiteIntake(
    normalized,
    {
      repositories: fake.repositories,
      actorContext: {
        emailEvidence: 'authenticated',
        allowPersonCreation: true,
      },
      personPolicy: { allowCreation: true, role: 'buyer' },
      createId: (() => {
        const ids = [PERSON, INTERACTION]
        return () => ids.shift()
      })(),
      now: () => new Date(NOW),
    },
    { trustedResolutionRetry: true },
  )
  assert.equal(fake.calls.creates, 1)
  assert.equal(fake.calls.canonical, 1)
  assert.equal(fake.receipts.get(SUBMISSION).status, 'completed')
}

// Same UUID with a changed payload is rejected before claiming.
{
  const fake = fakeSystem()
  fake.receipts.set(SUBMISSION, receipt(normalized))
  const changed = { ...normalized, message: 'Different message' }
  const result = await processWebsiteIntake(changed, {
    repositories: fake.repositories,
  })
  assert.equal(result.status, 'invalid')
  assert.equal(fake.calls.claims, 0)
}

// Concurrent duplicates have one processing winner in the repository contract.
{
  const fake = fakeSystem({ identities: [['email:buyer@example.com', PERSON]] })
  const [left, right] = await Promise.all([
    processWebsiteIntake(normalized, { repositories: fake.repositories }),
    processWebsiteIntake(normalized, { repositories: fake.repositories }),
  ])
  assert.equal(left.accepted && right.accepted, true)
  assert.equal(fake.calls.claims, 1)
  assert.equal(fake.calls.canonical, 1)
}

// Once a stale claim is reclaimed, the earlier worker's timestamp token can no
// longer finalize the receipt; only the current owner can transition it.
{
  const fake = fakeSystem({ staleProcessing: true })
  fake.receipts.set(
    SUBMISSION,
    receipt(normalized, 'processing', {
      processingStartedAt: '2026-08-18T15:00:00.000Z',
    }),
  )
  const workerA = fake.receipts.get(SUBMISSION).processingStartedAt
  const workerBReceipt = await fake.repositories.claimReceipt(SUBMISSION)
  assert.notEqual(workerBReceipt.processingStartedAt, workerA)
  assert.equal(
    await fake.repositories.transitionReceipt({
      submissionId: SUBMISSION,
      claimToken: workerA,
      from: 'processing',
      to: 'completed',
      interactionId: INTERACTION,
    }),
    false,
  )
  assert.equal(
    await fake.repositories.transitionReceipt({
      submissionId: SUBMISSION,
      claimToken: workerBReceipt.processingStartedAt,
      from: 'processing',
      to: 'completed',
      interactionId: INTERACTION,
    }),
    true,
  )
}

// Completion recovery links an already-created source interaction and does not
// re-run canonical persistence.
{
  const fake = fakeSystem()
  fake.receipts.set(SUBMISSION, receipt(normalized))
  fake.interactions.set(`website:${SUBMISSION}`, {
    id: INTERACTION,
    personId: PERSON,
    propertyId: PROPERTY,
  })
  await processWebsiteIntake(normalized, { repositories: fake.repositories })
  assert.equal(fake.receipts.get(SUBMISSION).status, 'completed')
  assert.equal(fake.receipts.get(SUBMISSION).interactionId, INTERACTION)
  assert.equal(fake.calls.canonical, 0)
}

// Both action types produce canonical work; transient failure returns to received.
for (const requestType of ['private_viewing', 'property_information']) {
  const fake = fakeSystem({ identities: [['email:buyer@example.com', PERSON]] })
  await processWebsiteIntake(payload({ requestType }), {
    repositories: fake.repositories,
  })
  assert.equal(fake.calls.canonical, 1)
}
{
  const fake = fakeSystem({
    identities: [['email:buyer@example.com', PERSON]],
    transientFailure: true,
  })
  await assert.rejects(
    processWebsiteIntake(normalized, { repositories: fake.repositories }),
    /transient/,
  )
  assert.equal(fake.receipts.get(SUBMISSION).status, 'received')
}

// Unexpected failures remain processing for safe stale-claim recovery.
{
  const fake = fakeSystem({ identities: [['email:buyer@example.com', PERSON]] })
  fake.repositories.persistCanonical = async () => {
    throw new Error('unexpected')
  }
  await assert.rejects(
    processWebsiteIntake(normalized, { repositories: fake.repositories }),
    /unexpected/,
  )
  assert.equal(fake.receipts.get(SUBMISSION).status, 'processing')
}

// Repository query verification uses injected executors only.
{
  const statements = []
  const execute = async (strings, ...values) => {
    statements.push({ text: strings.join('?'), values })
    return []
  }
  await claimWebsiteIntakeReceipt(SUBMISSION, {}, execute)
  assert.match(statements[0].text, /interval '15 minutes'/)
  assert.match(statements[0].text, /status = 'received'/)
  await assert.rejects(
    transitionWebsiteIntakeReceipt(
      {
        submissionId: SUBMISSION,
        claimToken: NOW,
        from: 'received',
        to: 'completed',
      },
      execute,
    ),
    /not allowed/,
  )
  await assert.rejects(
    transitionWebsiteIntakeReceipt(
      {
        submissionId: SUBMISSION,
        claimToken: NOW,
        from: 'processing',
        to: 'completed',
      },
      execute,
    ),
    /interaction ID/,
  )
  await transitionWebsiteIntakeReceipt(
    {
      submissionId: SUBMISSION,
      claimToken: NOW,
      from: 'processing',
      to: 'received',
    },
    execute,
  )
  assert.match(statements.at(-1).text, /processing_started_at = /)
}

{
  const statements = []
  const transaction = async (build) => {
    const execute = async (strings, ...values) => {
      statements.push({ text: strings.join('?'), values })
      return statements.length === 1 ? [{ id: INTERACTION }] : [{ id: 'row' }]
    }
    return Promise.all(build(execute))
  }
  const result = await persistCanonicalWebsiteIntake(
    {
      interactionId: INTERACTION,
      personId: PERSON,
      propertyId: PROPERTY,
      submissionId: SUBMISSION,
      requestType: 'private_viewing',
      occurredAt: NOW,
      displayName: 'María Rivera',
      email: 'buyer@example.com',
    },
    transaction,
    async () => {
      throw new Error('Duplicate lookup must not run after a created interaction.')
    },
  )
  assert.equal(result.created, true)
  assert.equal(statements.length, 3)
  assert.match(statements[1].text, /where exists/)
  assert.match(statements[1].text, /on conflict \(person_id, property_id\) do nothing/)
  assert.match(statements[2].text, /source_interaction_id/)
  assert.match(statements[2].text, /'human'/)
}

// A source-identity conflict yields no downstream rows; lookup resolves the
// existing interaction. The SQL guards make the interest/task inserts no-ops.
{
  const statements = []
  const transaction = async (build) => {
    const execute = async (strings, ...values) => {
      statements.push({ text: strings.join('?'), values })
      return []
    }
    return Promise.all(build(execute))
  }
  const result = await persistCanonicalWebsiteIntake(
    {
      interactionId: '50000000-0000-4000-8000-000000000099',
      personId: PERSON,
      propertyId: PROPERTY,
      submissionId: SUBMISSION,
      requestType: 'property_information',
      occurredAt: NOW,
      displayName: 'María Rivera',
      email: 'buyer@example.com',
    },
    transaction,
    async () => [{ id: INTERACTION }],
  )
  assert.deepEqual(result, { interactionId: INTERACTION, created: false })
  assert.equal(statements.length, 3)
  assert.match(statements[1].text, /where exists/)
  assert.match(statements[2].text, /where exists/)
}

// Canonical effects share one transaction boundary; a transaction rejection
// surfaces without attempting the duplicate lookup.
{
  let lookupCalls = 0
  await assert.rejects(
    persistCanonicalWebsiteIntake(
      {
        interactionId: INTERACTION,
        personId: PERSON,
        propertyId: PROPERTY,
        submissionId: SUBMISSION,
        requestType: 'private_viewing',
        occurredAt: NOW,
        displayName: 'María Rivera',
        email: 'buyer@example.com',
      },
      async () => {
        throw new Error('transaction rolled back')
      },
      async () => {
        lookupCalls += 1
        return []
      },
    ),
    /rolled back/,
  )
  assert.equal(lookupCalls, 0)
}

console.log('CRM-04 website intake verification passed (zero Neon access).')

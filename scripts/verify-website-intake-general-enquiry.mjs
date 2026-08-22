import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

// ---------------------------------------------------------------------------
// PX-26 Generic Contact → CRM Intake — scoped verification, zero Neon access.
//
// Proves the contract-extension approach: the single website_intake_submission
// receipt now carries a property-less general_enquiry request type (migration
// 011) with NO synthetic property and NO property_interest side effect, while
// the property-scoped private_viewing / property_information contract is
// unchanged (still requires propertyId). Identity/idempotency semantics
// (source_system='website', source_external_id=submissionId) are preserved.
// ---------------------------------------------------------------------------

const {
  adaptWebsiteIntake,
  normalizeWebsiteIntake,
  parseWebsiteIntakeFormData,
  processWebsiteIntake,
} = await import('../lib/website-intake.ts')
const { persistCanonicalWebsiteIntake } = await import('../db/website-intake.ts')

const SUBMISSION = '40000000-0000-4000-8000-000000000026'
const PROPERTY = '40000000-0000-4000-8000-000000000001'
const PERSON = '10000000-0000-4000-8000-000000000001'
const INTERACTION = '50000000-0000-4000-8000-000000000026'
const NOW = '2026-08-26T12:00:00.000Z'
const EMAIL = 'buyer@example.com'
const NAME = 'María Rivera'

function generalEnquiry(overrides = {}) {
  return normalizeWebsiteIntake({
    submissionId: SUBMISSION,
    requestType: 'general_enquiry',
    name: NAME,
    email: EMAIL,
    message: 'Tell me about island life.',
    ...overrides,
  })
}

function propertyScoped(requestType, overrides = {}) {
  return normalizeWebsiteIntake({
    submissionId: SUBMISSION,
    requestType,
    propertyId: PROPERTY,
    name: NAME,
    email: EMAIL,
    message: 'I would like to visit.',
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
    claims: 0,
    canonical: 0,
    canonicalInputs: [],
    findActiveProperty: 0,
    creates: 0,
    transitions: [],
  }
  let claimSequence = 0

  const crm = {
    async findInteractionBySourceIdentity(system, externalId) {
      return interactions.get(`${system}:${externalId}`) ?? null
    },
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
      calls.findActiveProperty += 1
      return id === PROPERTY ? { id } : null
    },
    async insertOrReadReceipt(input) {
      const existing = receipts.get(input.submissionId)
      if (existing) return { receipt: existing, created: false }
      const created = receipt(input)
      receipts.set(input.submissionId, created)
      return { receipt: created, created: true }
    },
    async claimReceipt(id) {
      const current = receipts.get(id)
      if (!current || current.status !== 'received') return null
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
      calls.canonicalInputs.push(input)
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

// ---------------------------------------------------------------------------
// 1. Contract: general_enquiry with a propertyId is rejected; the property-less
//    form normalizes without inventing a property.
// ---------------------------------------------------------------------------
{
  assert.throws(
    () =>
      generalEnquiry({
        propertyId: PROPERTY,
      }),
    /must not include a property ID/,
  )
  // An empty/blank propertyId is treated as absent, so a property-less general
  // enquiry still normalizes (the front end never sends one, and the backend
  // refuses a real property id).
  const propertyLess = generalEnquiry()
  assert.equal(propertyLess.requestType, 'general_enquiry')
  assert.equal(propertyLess.propertyId, undefined)
  assert.equal(propertyLess.displayName, NAME)
  assert.equal(propertyLess.email, EMAIL)
  assert.equal(propertyLess.message, 'Tell me about island life.')
}

// ---------------------------------------------------------------------------
// 2. Contract: property-scoped requests still REQUIRE propertyId — the
//    private_viewing / property_information contract is not weakened.
// ---------------------------------------------------------------------------
{
  for (const requestType of ['private_viewing', 'property_information']) {
    assert.throws(
      () =>
        normalizeWebsiteIntake({
          submissionId: SUBMISSION,
          requestType,
          name: NAME,
          email: EMAIL,
        }),
      /Property ID is required/,
    )
    const scoped = propertyScoped(requestType)
    assert.equal(scoped.propertyId, PROPERTY)
    assert.throws(() => propertyScoped(requestType, { propertyId: 'not-a-uuid' }), /UUID/)
  }
  // A general enquiry can never be smuggled through the property rule.
  assert.throws(
    () =>
      normalizeWebsiteIntake({
        submissionId: SUBMISSION,
        requestType: 'seller',
        name: NAME,
        email: EMAIL,
      }),
    /invalid/,
  )
}

// ---------------------------------------------------------------------------
// 3. Form parsing: the property-less /contact form maps to a general enquiry;
//    a forged property id is refused before any write.
// ---------------------------------------------------------------------------
{
  const form = new FormData()
  form.set('submissionId', SUBMISSION)
  form.set('requestType', 'general_enquiry')
  form.set('name', NAME)
  form.set('email', EMAIL)
  form.set('message', 'Tell me about island life.')
  const parsed = parseWebsiteIntakeFormData(form)
  assert.equal(parsed.honeypot, false)
  assert.equal(parsed.payload.requestType, 'general_enquiry')
  assert.equal(parsed.payload.propertyId, undefined)

  const forged = new FormData()
  forged.set('submissionId', SUBMISSION)
  forged.set('requestType', 'general_enquiry')
  forged.set('propertyId', PROPERTY)
  forged.set('name', NAME)
  forged.set('email', EMAIL)
  assert.throws(() => parseWebsiteIntakeFormData(forged), /must not include a property ID/)
}

// ---------------------------------------------------------------------------
// 4. Adaptation: general_enquiry maps to the canonical inbound event with no
//    property context and no requested action; the source identity stays
//    (website, submissionId).
// ---------------------------------------------------------------------------
{
  const adapted = adaptWebsiteIntake(generalEnquiry(), NOW)
  assert.equal(adapted.event.eventType, 'general_enquiry_submitted')
  assert.deepEqual(adapted.event.source, { system: 'website', externalId: SUBMISSION })
  assert.equal(adapted.event.channel, 'website')
  assert.equal(adapted.event.direction, 'inbound')
  assert.deepEqual(adapted.event.context, {})
  assert.equal(adapted.event.intentHints, undefined)
  assert.equal(adapted.event.content.subject, 'General enquiry')
  assert.equal(adapted.event.actor.identityHints[0].kind, 'email')
  assert.equal(adapted.event.actor.identityHints[0].value, EMAIL)
  assert.equal(adapted.event.actor.identityHints[0].evidence, 'user_supplied')
  assert.equal(adapted.event.rawMetadata.requestType, 'general_enquiry')

  // Property-scoped adaptation still carries context + intent (unchanged).
  const scoped = adaptWebsiteIntake(propertyScoped('private_viewing'), NOW)
  assert.equal(scoped.event.context.propertyId, PROPERTY)
  assert.equal(scoped.event.intentHints.requestedAction, 'private_viewing')
}

// ---------------------------------------------------------------------------
// 5. Full pipeline: a property-less general enquiry with a known identity
//    persists the canonical interaction + receipt completion WITHOUT ever
//    looking up (or fabricating) a property.
// ---------------------------------------------------------------------------
{
  const fake = fakeSystem({ identities: [['email:buyer@example.com', PERSON]] })
  const result = await processWebsiteIntake(generalEnquiry(), {
    repositories: fake.repositories,
    createId: () => INTERACTION,
    now: () => new Date(NOW),
  })
  assert.deepEqual(result, { accepted: true, status: 'accepted' })
  assert.equal(fake.calls.findActiveProperty, 0, 'property-less intake must not resolve a property')
  assert.equal(fake.calls.canonical, 1)
  assert.equal(fake.calls.creates, 0)
  const canonicalInput = fake.calls.canonicalInputs[0]
  assert.equal(canonicalInput.propertyId, undefined)
  assert.equal(canonicalInput.requestType, 'general_enquiry')
  assert.equal(canonicalInput.submissionId, SUBMISSION)
  assert.equal(canonicalInput.interactionId, INTERACTION)
  const completed = fake.receipts.get(SUBMISSION)
  assert.equal(completed.status, 'completed')
  assert.equal(completed.interactionId, INTERACTION)

  // Public replay is a no-op: idempotency preserved by the receipt contract.
  await processWebsiteIntake(generalEnquiry(), {
    repositories: fake.repositories,
    createId: () => INTERACTION,
  })
  assert.equal(fake.calls.claims, 1)
  assert.equal(fake.calls.canonical, 1)
}

// ---------------------------------------------------------------------------
// 6. Canonical persistence, property-less: interaction + follow-up task only —
//    NO property_interest row is ever emitted, and no property id is written.
// ---------------------------------------------------------------------------
{
  const statements = []
  const transaction = async (build) => {
    const execute = async (strings, ...values) => {
      statements.push({ text: strings.join('?').replace(/\s+/g, ' ').trim(), values })
      if (statements.length === 1) return [{ id: INTERACTION }]
      return [{ id: 'task-1' }]
    }
    return Promise.all(build(execute))
  }
  const result = await persistCanonicalWebsiteIntake(
    {
      interactionId: INTERACTION,
      personId: PERSON,
      submissionId: SUBMISSION,
      requestType: 'general_enquiry',
      occurredAt: NOW,
      displayName: NAME,
      email: EMAIL,
      message: 'Tell me about island life.',
    },
    transaction,
    async () => {
      throw new Error('Duplicate lookup must not run after a created interaction.')
    },
  )
  assert.deepEqual(result, { interactionId: INTERACTION, created: true })
  assert.equal(statements.length, 2, 'property-less intake must issue exactly interaction + task inserts')

  const [interaction, task] = statements
  assert.match(interaction.text, /insert into interaction/)
  assert.doesNotMatch(interaction.text, /property_interest/)
  // values = [interactionId, personId, propertyId(null), eventType, occurredAt,
  //           title, message, submissionId, metadata]
  assert.equal(interaction.values[2], null, 'property-less interaction must write a null property_id')
  assert.equal(interaction.values[3], 'general_enquiry_submitted')
  assert.equal(interaction.values[5], 'General enquiry')
  assert.equal(interaction.values[7], SUBMISSION)
  assert.match(interaction.text, /source_system, source_external_id/)
  assert.match(interaction.text, /on conflict \(source_system, source_external_id\)/)

  assert.match(task.text, /insert into task/)
  assert.match(task.text, /'human'/)
  // values = [taskTitle, message, personId, propertyId(null), interactionId, interactionId]
  assert.equal(task.values[0], `Follow up on general enquiry from ${NAME}`)
  assert.equal(task.values[2], PERSON)
  assert.equal(task.values[3], null, 'follow-up task must not carry a property id')
  assert.equal(task.values[4], INTERACTION, 'follow-up task must reference the canonical interaction')
}

// ---------------------------------------------------------------------------
// 7. Canonical persistence, property-scoped: the property_interest insert is
//    preserved — the property enquiry contract is not weakened.
// ---------------------------------------------------------------------------
{
  const statements = []
  const transaction = async (build) => {
    const execute = async (strings, ...values) => {
      statements.push({ text: strings.join('?').replace(/\s+/g, ' ').trim(), values })
      return [{ id: 'row' }]
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
      displayName: NAME,
      email: EMAIL,
    },
    transaction,
    async () => [{ id: INTERACTION }],
  )
  assert.equal(result.created, true)
  assert.equal(statements.length, 3)
  const interest = statements[1]
  assert.match(interest.text, /insert into property_interest/)
  assert.match(interest.text, /on conflict \(person_id, property_id\) do nothing/)
  assert.equal(interest.values[0], PERSON)
  assert.equal(interest.values[1], PROPERTY)
}

// ---------------------------------------------------------------------------
// 8. Needs-review surfacing: the read projection is a property LEFT JOIN with
//    the same status filter — property-less general enquiries surface with
//    null property context, and the UI labels the request type.
// ---------------------------------------------------------------------------
{
  const projection = await readFile(
    new URL('../db/needs-review.ts', import.meta.url),
    'utf8',
  )
  assert.match(projection, /left join property/)
  assert.match(projection, /on property\.id = w\.property_id/)
  assert.match(projection, /where w\.status in \('received', 'resolution_required'\)/)
  assert.doesNotMatch(projection, /where w\.property_id is not null/)

  const component = await readFile(
    new URL('../components/portal/needs-review.tsx', import.meta.url),
    'utf8',
  )
  assert.match(component, /case "general_enquiry"/)
}

// ---------------------------------------------------------------------------
// 9. Migration 011 records the CHECK contract in the schema.
// ---------------------------------------------------------------------------
{
  const migration = await readFile(
    new URL('../db/migrations/011_website_intake_general_enquiry.sql', import.meta.url),
    'utf8',
  )
  assert.match(
    migration,
    /request_type in \(\s*'private_viewing',\s*'property_information',\s*'general_enquiry'\s*\)/,
  )
  assert.match(migration, /request_type = 'general_enquiry'/)
  assert.match(migration, /property_id is null/)
  assert.match(migration, /request_type in \('private_viewing', 'property_information'\)/)
  assert.match(migration, /property_id is not null/)
}

console.log('PX-26 generic contact → CRM intake verification passed (zero Neon access).')

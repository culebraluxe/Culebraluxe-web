import assert from 'node:assert/strict'

const { adaptCommunicationsEvent } = await import('../lib/crm-communications-normalization.ts')
const { prepareCommunicationsIntake } = await import('../lib/crm-communications-intake.ts')
const { sanitizeRawMetadata } = await import('../lib/crm-intake-normalization.ts')

const PERSON = '10000000-0000-4000-8000-000000000001'
const PROPERTY = '20000000-0000-4000-8000-000000000001'
const PROPERTY_B = '20000000-0000-4000-8000-000000000002'
const DEAL = '40000000-0000-4000-8000-000000000001'
const INTERACTION = '30000000-0000-4000-8000-000000000001'
const OWNED = '+17875550100'
const OWNED_TWO = '+17875550101'
const ACTOR = '+17875550199'
const ACTOR_TWO = '+17875550198'
const SHARED = '+17875550999'

const configuration = {
  ownedLines: [{ phone: OWNED, creationRole: 'buyer' }],
  sharedExternalPhones: [SHARED],
  systemEndpoints: ['CulebraLuxe', '12345'],
}

function event(overrides = {}) {
  return {
    provider: 'Fixture',
    accountNamespace: 'Sales_One',
    transport: 'sms',
    providerEventId: 'Event A',
    occurredAt: '2026-08-18T12:00:00.000Z',
    from: [{ kind: 'address', value: ACTOR }],
    to: [{ kind: 'address', value: OWNED }],
    trustedDirection: 'inbound',
    actorAssurance: 'transport_observed',
    plainText: 'Please tell me more.',
    ...overrides,
  }
}

function interaction() {
  return {
    id: INTERACTION,
    personId: PERSON,
    channel: 'sms',
    eventType: 'sms_received',
    occurredAt: '2026-08-18T12:00:00.000Z',
    sourceSystem: 'communications:fixture:sales_one',
    sourceExternalId: 'sms:Event A',
    sourceMetadata: {},
    createdAt: '2026-08-18T12:00:01.000Z',
  }
}

function fakeRepositories(options = {}) {
  const createdPeople = new Set()
  const calls = {
    duplicate: 0,
    person: 0,
    match: 0,
    ownership: 0,
    create: 0,
    property: 0,
    slug: 0,
    deal: 0,
    order: [],
    createInputs: [],
  }
  return {
    calls,
    repositories: {
      async findInteractionBySourceIdentity(system, externalId) {
        calls.duplicate += 1
        calls.order.push(`duplicate:${system}:${externalId}`)
        return options.duplicate ?? null
      },
      async personExists(personId) {
        calls.person += 1
        calls.order.push('person')
        return personId === PERSON || createdPeople.has(personId)
      },
      async findIdentityMatch(hint) {
        calls.match += 1
        calls.order.push(`match:${hint.normalizedValue}`)
        if (options.existingPhone === hint.normalizedValue) {
          return { identityId: 'identity-1', personId: PERSON, kind: hint.kind, normalizedValue: hint.normalizedValue }
        }
        return null
      },
      async findIdentityOwnership(hint) {
        calls.ownership += 1
        calls.order.push(`ownership:${hint.normalizedValue}`)
        if (options.archivedPhone === hint.normalizedValue) {
          return { identityId: 'identity-archived', personId: PERSON, kind: hint.kind, normalizedValue: hint.normalizedValue, archived: true }
        }
        if (options.existingPhone === hint.normalizedValue) {
          return { identityId: 'identity-1', personId: PERSON, kind: hint.kind, normalizedValue: hint.normalizedValue, archived: false }
        }
        return null
      },
      async createPersonWithIdentities(input) {
        calls.create += 1
        calls.order.push('create')
        calls.createInputs.push(input)
        if (options.forbidCreate) throw new Error('Unexpected person creation')
        createdPeople.add(input.personId)
      },
      async findPropertyById(id) {
        calls.property += 1
        calls.order.push('property')
        return id === PROPERTY ? { id: PROPERTY, slug: 'casa-luar' } : null
      },
      async findPropertyBySlug(slug) {
        calls.slug += 1
        calls.order.push('slug')
        if (slug === 'casa-luar') return { id: PROPERTY, slug }
        if (slug === 'villa-mar-azul') return { id: PROPERTY_B, slug }
        return null
      },
      async findDealById(id) {
        calls.deal += 1
        calls.order.push('deal')
        return id === DEAL
          ? { id: DEAL, personId: options.dealPersonId ?? PERSON, propertyId: options.dealPropertyId ?? PROPERTY }
          : null
      },
    },
  }
}

function rejected(input, config = configuration, pattern) {
  const result = adaptCommunicationsEvent(input, config)
  assert.equal(result.status, 'rejected')
  if (pattern) assert.match(result.reason, pattern)
}

// Seven exact event types and the complete terminal call matrix.
for (const [transport, direction, eventType] of [
  ['sms', 'inbound', 'sms_received'],
  ['sms', 'outbound', 'sms_sent'],
  ['imessage', 'inbound', 'imessage_received'],
  ['imessage', 'outbound', 'imessage_sent'],
]) {
  const outbound = direction === 'outbound'
  const result = adaptCommunicationsEvent(event({
    transport,
    from: [{ kind: 'address', value: outbound ? OWNED : ACTOR }],
    to: [{ kind: 'address', value: outbound ? ACTOR : OWNED }],
    trustedDirection: direction,
  }), configuration)
  assert.equal(result.status, 'accepted')
  assert.equal(result.inboundEvent.eventType, eventType)
  assert.equal(result.inboundEvent.channel, transport)
}

const callMatrix = [
  ['inbound', 'connected', 12, 'call_received', false],
  ['inbound', 'no_answer', undefined, 'call_missed', false],
  ['inbound', 'busy', undefined, 'call_missed', false],
  ['inbound', 'voicemail', undefined, 'call_missed', true],
  ['outbound', 'connected', 0, 'call_placed', false],
  ['outbound', 'voicemail', 9, 'call_placed', true],
  ['outbound', 'no_answer', undefined, 'call_placed', false],
  ['outbound', 'busy', undefined, 'call_placed', false],
  ['outbound', 'failed', undefined, 'call_placed', false],
  ['outbound', 'canceled', undefined, 'call_placed', false],
]
for (const [direction, disposition, durationSeconds, eventType, voicemail] of callMatrix) {
  const outbound = direction === 'outbound'
  const result = adaptCommunicationsEvent(event({
    transport: 'call',
    plainText: undefined,
    from: [{ kind: 'address', value: outbound ? OWNED : ACTOR }],
    to: [{ kind: 'address', value: outbound ? ACTOR : OWNED }],
    trustedDirection: direction,
    callDisposition: disposition,
    durationSeconds,
  }), configuration)
  assert.equal(result.status, 'accepted')
  assert.equal(result.inboundEvent.eventType, eventType)
  assert.equal(result.inboundEvent.rawMetadata.callDisposition, disposition)
  assert.equal(result.inboundEvent.rawMetadata.voicemail, voicemail || undefined)
  assert.equal(result.inboundEvent.rawMetadata.durationProvenance, durationSeconds === undefined ? undefined : 'provider_reported')
}
for (const disposition of ['failed', 'canceled']) {
  rejected(event({ transport: 'call', plainText: undefined, callDisposition: disposition }), configuration, /transport failure/)
}
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'connected' }), configuration, /requires/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'no_answer', durationSeconds: 1 }), configuration, /forbids/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'connected', durationSeconds: -1 }), configuration, /non-negative safe-integer/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'connected', durationSeconds: 1.5 }), configuration, /non-negative safe-integer/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'connected', durationSeconds: Number.MAX_SAFE_INTEGER + 1 }), configuration, /non-negative safe-integer/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'voicemail', durationSeconds: 1 }), configuration, /forbids/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'voicemail', durationSeconds: undefined, from: [{ kind: 'address', value: OWNED }], to: [{ kind: 'address', value: ACTOR }], trustedDirection: 'outbound' }), configuration, /requires/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'busy', durationSeconds: 1 }), configuration, /forbids/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'failed', durationSeconds: 1, from: [{ kind: 'address', value: OWNED }], to: [{ kind: 'address', value: ACTOR }], trustedDirection: 'outbound' }), configuration, /forbids/)
rejected(event({ transport: 'call', plainText: undefined, callDisposition: 'magic' }), configuration, /invalid/)
rejected(event({ transport: 'call', plainText: 'transcript', callDisposition: 'connected', durationSeconds: 1 }), configuration, /cannot contain message/)
rejected(event({ transport: 'sms', callDisposition: 'connected' }), configuration, /cannot contain call/)

// Raw endpoints cannot claim classification; configuration is validated and authoritative.
assert.equal(adaptCommunicationsEvent(event({ from: [{ kind: 'address', value: '12345' }] }), configuration).status, 'excluded')
assert.equal(adaptCommunicationsEvent(event({ from: [{ kind: 'address', value: SHARED }] }), configuration).status, 'resolution_required')
assert.equal(adaptCommunicationsEvent(event({ from: [{ kind: 'withheld' }] }), configuration).status, 'resolution_required')
rejected(event({ from: [{ kind: 'address', value: '7875550199' }] }), configuration, /strict E.164/)
rejected(event({ from: [{ kind: 'address', value: '+17875550199 ext 2' }] }), configuration, /strict E.164/)
rejected(event({ from: [{ kind: 'address', value: 'ALPHA' }] }), configuration, /strict E.164/)
rejected(event({ from: [{ kind: 'address', value: '+12' }] }), configuration, /strict E.164/)
rejected(event({ from: [{ kind: 'address', value: ACTOR }], to: [{ kind: 'address', value: ACTOR_TWO }] }), configuration, /missing_owned/)
assert.equal(adaptCommunicationsEvent(event({ from: [{ kind: 'address', value: ACTOR }, { kind: 'address', value: ACTOR_TWO }] }), configuration).status, 'resolution_required')
assert.equal(adaptCommunicationsEvent(event({ to: [{ kind: 'address', value: OWNED }, { kind: 'address', value: ACTOR_TWO }] }), configuration).status, 'resolution_required')
assert.equal(adaptCommunicationsEvent(event({ from: [{ kind: 'address', value: OWNED }], to: [{ kind: 'address', value: OWNED_TWO }], trustedDirection: undefined }), { ownedLines: [{ phone: OWNED }, { phone: OWNED_TWO }] }).status, 'excluded')
assert.equal(adaptCommunicationsEvent(event({ from: [{ kind: 'address', value: OWNED }, { kind: 'address', value: ACTOR_TWO }], to: [{ kind: 'address', value: OWNED_TWO }], trustedDirection: undefined }), { ownedLines: [{ phone: OWNED }, { phone: OWNED_TWO }] }).status, 'resolution_required')
rejected(event({ trustedDirection: 'outbound' }), configuration, /conflicting/)
rejected(event(), { ownedLines: [{ phone: OWNED, creationRole: 'buyer' }, { phone: OWNED, creationRole: 'seller' }] }, /conflicting roles/)
rejected(event(), { ownedLines: [{ phone: OWNED, creationRole: 'buyer' }, { phone: '+1 (787) 555-0100', creationRole: 'buyer' }] }, /duplicate phone/)
rejected(event(), { ownedLines: [{ phone: OWNED }], sharedExternalPhones: [OWNED] }, /conflicting categories/)
rejected(event(), { ownedLines: [{ phone: OWNED }], systemEndpoints: [OWNED] }, /conflicting categories/)

// Source grammar and opaque identifiers preserve permitted case/internal whitespace.
{
  const result = adaptCommunicationsEvent(event({ provider: ' FIXTURE ', accountNamespace: ' Sales_One ', providerEventId: 'Case ID A', correlationId: 'Thread ID A' }), configuration)
  assert.equal(result.status, 'accepted')
  assert.equal(result.inboundEvent.source.system, 'communications:fixture:sales_one')
  assert.equal(result.inboundEvent.source.externalId, 'sms:Case ID A')
  assert.equal(result.inboundEvent.rawMetadata.correlationId, 'Thread ID A')
}
{
  const first = adaptCommunicationsEvent(event({ correlationId: 'Conversation A' }), configuration)
  const retry = adaptCommunicationsEvent(event({ correlationId: 'Conversation B' }), configuration)
  assert.equal(first.status, 'accepted')
  assert.equal(retry.status, 'accepted')
  assert.equal(first.inboundEvent.source.system, retry.inboundEvent.source.system)
  assert.equal(first.inboundEvent.source.externalId, retry.inboundEvent.source.externalId)
  assert.notEqual(first.inboundEvent.rawMetadata.correlationId, retry.inboundEvent.rawMetadata.correlationId)
}
for (const overrides of [
  { provider: 'bad provider' },
  { provider: 'x'.repeat(65) },
  { providerEventId: '' },
  { providerEventId: ` ${'a'}` },
  { providerEventId: 'a:' },
  { providerEventId: 'https://example.com/id' },
  { providerEventId: 'a\u0000b' },
  { providerEventId: 'x'.repeat(513) },
  { correlationId: 'bad:id' },
]) rejected(event(overrides), configuration)
assert.equal(adaptCommunicationsEvent(event({ providerEventId: 'access_token' }), configuration).status, 'accepted')

// Message normalization is Unicode-code-point based and content remains summary-only.
{
  const result = adaptCommunicationsEvent(event({ plainText: '  Ｈｅｌｌｏ\r\nCulebra  ' }), configuration)
  assert.equal(result.status, 'accepted')
  assert.equal(result.inboundEvent.content.summary, 'Hello\nCulebra')
  assert.equal('plainText' in result.inboundEvent.rawMetadata, false)
}
for (const plainText of ['', ' \n ', 'bad\u0000text', 'x'.repeat(4001)]) {
  rejected(event({ plainText }), configuration)
}
assert.equal(adaptCommunicationsEvent(event({ plainText: '😀'.repeat(4000) }), configuration).status, 'accepted')

// Closed metadata; shared sanitizer retains recursive secrets and serialized-size limits.
{
  const result = adaptCommunicationsEvent(event({ providerPayload: { apiKey: 'ignored' }, recordingUrl: 'ignored' }), configuration)
  assert.equal(result.status, 'accepted')
  assert.deepEqual(Object.keys(result.inboundEvent.rawMetadata).sort(), ['transport'])
}
assert.throws(() => sanitizeRawMetadata({ nested: { authorization: 'secret' } }), /prohibited secret/)
assert.throws(() => sanitizeRawMetadata({ value: 'x'.repeat(33 * 1024) }), /32 KB/)

// Coordinator idempotency, cross-channel identity, assurance, role, and no-write boundary.
{
  const { repositories, calls } = fakeRepositories({ duplicate: interaction(), forbidCreate: true })
  const result = await prepareCommunicationsIntake(event(), configuration, repositories)
  assert.equal(result.status, 'duplicate')
  assert.equal(result.existingInteractionId, INTERACTION)
  assert.equal(calls.duplicate, 1)
  assert.equal(calls.match + calls.ownership + calls.property + calls.deal + calls.create, 0)
}
for (const transport of ['call', 'sms', 'imessage']) {
  const { repositories } = fakeRepositories({ existingPhone: ACTOR, forbidCreate: true })
  const result = await prepareCommunicationsIntake(event({
    transport,
    plainText: transport === 'call' ? undefined : 'Hello',
    callDisposition: transport === 'call' ? 'connected' : undefined,
    durationSeconds: transport === 'call' ? 1 : undefined,
  }), configuration, repositories)
  assert.equal(result.status, 'ready')
  assert.equal(result.personResult.personId, PERSON)
  assert.equal(result.intakeResult.interactionInput.personId, PERSON)
  assert.equal(result.intakeResult.followUpIntent, undefined)
  assert.equal(result.intakeResult.propertyInterestIntent, undefined)
  assert.equal(result.intakeResult.normalizedEvent.intentHints, undefined)
}
{
  const { repositories, calls } = fakeRepositories()
  const result = await prepareCommunicationsIntake(event({ displayNameHint: 'Known Client Name' }), configuration, repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(calls.create, 0)
  assert.equal(calls.match, 1)
  assert.match(calls.order.find((entry) => entry.startsWith('match:')), new RegExp(`^match:${ACTOR.replace('+', '\\+')}$`))
  assert.equal(calls.order.some((entry) => entry.includes('Known Client Name')), false)
}
for (const actorAssurance of ['ownership_verified', 'authenticated_actor']) {
  const { repositories, calls } = fakeRepositories()
  const result = await prepareCommunicationsIntake(event({ actorAssurance }), configuration, repositories)
  assert.equal(result.status, 'ready')
  assert.equal(result.personResult.status, 'created')
  assert.equal(calls.create, 1)
  assert.equal(calls.createInputs[0].role, 'buyer')
  assert.equal(calls.createInputs[0].identities[0].normalizedValue, ACTOR)
}
{
  const { repositories, calls } = fakeRepositories()
  const result = await prepareCommunicationsIntake(event({ actorAssurance: 'ownership_verified' }), { ownedLines: [{ phone: OWNED }] }, repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(calls.create, 0)
}
{
  const config = { ownedLines: [{ phone: OWNED, creationRole: 'buyer' }, { phone: OWNED_TWO, creationRole: 'seller' }] }
  const { repositories, calls } = fakeRepositories()
  const result = await prepareCommunicationsIntake(event({ actorAssurance: 'ownership_verified', to: [{ kind: 'address', value: OWNED }, { kind: 'address', value: OWNED_TWO }] }), config, repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(calls.create, 0)
}
{
  const { repositories, calls } = fakeRepositories()
  const result = await prepareCommunicationsIntake(event({
    actorAssurance: 'authenticated_actor',
    from: [{ kind: 'address', value: OWNED }],
    to: [{ kind: 'address', value: ACTOR }],
    trustedDirection: 'outbound',
  }), configuration, repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(calls.create, 0)
}
{
  const { repositories, calls } = fakeRepositories({ archivedPhone: ACTOR })
  const result = await prepareCommunicationsIntake(event({ actorAssurance: 'authenticated_actor' }), configuration, repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(calls.create, 0)
}

// Exact trusted context only; text that resembles a property never triggers lookup.
{
  const { repositories, calls } = fakeRepositories({ existingPhone: ACTOR, forbidCreate: true })
  const result = await prepareCommunicationsIntake(event({ plainText: 'See /properties/casa-luar' }), configuration, repositories)
  assert.equal(result.status, 'ready')
  assert.equal(calls.property + calls.slug + calls.deal, 0)
}
{
  const { repositories } = fakeRepositories({ existingPhone: ACTOR, forbidCreate: true })
  const result = await prepareCommunicationsIntake(event({ trustedContext: { propertyId: PROPERTY, propertySlug: 'villa-mar-azul' } }), configuration, repositories)
  assert.equal(result.status, 'rejected')
}
{
  const { repositories } = fakeRepositories({ existingPhone: ACTOR, forbidCreate: true, dealPropertyId: PROPERTY_B })
  const result = await prepareCommunicationsIntake(event({ trustedContext: { propertyId: PROPERTY, dealId: DEAL } }), configuration, repositories)
  assert.equal(result.status, 'rejected')
}

console.log('CRM communications intake fixture verification passed (zero Neon/provider/canonical writes).')

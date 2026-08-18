import assert from 'node:assert/strict'

const {
  extractRecognizedPropertySlug,
  normalizeDisplayName,
  normalizeEmail,
  normalizeIdentityHint,
  normalizePhone,
  sanitizeRawMetadata,
} = await import('../lib/crm-intake-normalization.ts')
const { prepareInboundEvent } = await import('../lib/crm-intake.ts')
const { getInteractionBySourceIdentity } = await import(
  '../db/interactions.ts'
)

const PERSON_A = '10000000-0000-4000-8000-000000000001'
const PERSON_B = '10000000-0000-4000-8000-000000000002'
const PROPERTY_A = '20000000-0000-4000-8000-000000000001'
const PROPERTY_B = '20000000-0000-4000-8000-000000000002'
const DEAL_A = '30000000-0000-4000-8000-000000000001'

function interaction(overrides = {}) {
  return {
    id: '40000000-0000-4000-8000-000000000001',
    personId: PERSON_A,
    channel: 'website',
    eventType: 'lead_submitted',
    occurredAt: '2026-08-18T12:00:00.000Z',
    sourceSystem: 'website',
    sourceExternalId: 'submission-1',
    sourceMetadata: {},
    createdAt: '2026-08-18T12:00:01.000Z',
    ...overrides,
  }
}

function inbound(overrides = {}) {
  return {
    source: { system: 'Website', externalId: 'submission-1' },
    occurredAt: '2026-08-18T08:00:00-04:00',
    channel: 'website',
    eventType: 'lead_submitted',
    direction: 'inbound',
    actor: {
      identityHints: [
        {
          kind: 'email',
          value: ' Buyer+Island@Example.com ',
          evidence: 'user_supplied',
        },
      ],
      displayNameHint: '  María   Rivera  ',
      roleHint: 'buyer',
    },
    content: { subject: 'Private viewing', summary: 'Interested in Casa Luar.' },
    context: { propertySlug: 'casa-luar' },
    intentHints: { requestedAction: 'private_viewing' },
    rawMetadata: { adapterVersion: 1 },
    ...overrides,
  }
}

function fakeRepositories(options = {}) {
  const calls = {
    duplicate: 0,
    person: 0,
    identity: 0,
    propertyId: 0,
    propertySlug: 0,
    deal: 0,
  }
  const identities =
    options.identities ??
    new Map([
      [
        'email:buyer+island@example.com',
        {
          identityId: '50000000-0000-4000-8000-000000000001',
          personId: PERSON_A,
        },
      ],
      [
        'phone:+17875550123',
        {
          identityId: '50000000-0000-4000-8000-000000000002',
          personId: PERSON_A,
        },
      ],
    ])

  return {
    calls,
    repositories: {
      async findInteractionBySourceIdentity() {
        calls.duplicate += 1
        return options.duplicate ?? null
      },
      async personExists(personId) {
        calls.person += 1
        return (options.people ?? [PERSON_A]).includes(personId)
      },
      async findIdentityMatch(hint) {
        calls.identity += 1
        const match = identities.get(`${hint.kind}:${hint.normalizedValue}`)
        return match
          ? {
              ...match,
              kind: hint.kind,
              normalizedValue: hint.normalizedValue,
            }
          : null
      },
      async findPropertyById(propertyId) {
        calls.propertyId += 1
        if (propertyId === PROPERTY_A) {
          return { id: PROPERTY_A, slug: 'casa-luar' }
        }
        if (propertyId === PROPERTY_B) {
          return { id: PROPERTY_B, slug: 'villa-mar-azul' }
        }
        return null
      },
      async findPropertyBySlug(slug) {
        calls.propertySlug += 1
        if (slug === 'casa-luar') {
          return { id: PROPERTY_A, slug: 'casa-luar' }
        }
        if (slug === 'villa-mar-azul') {
          return { id: PROPERTY_B, slug: 'villa-mar-azul' }
        }
        return null
      },
      async findDealById(dealId) {
        calls.deal += 1
        if (dealId !== DEAL_A) return null
        return {
          id: DEAL_A,
          personId: options.dealPersonId ?? PERSON_A,
          propertyId: options.dealPropertyId ?? PROPERTY_A,
        }
      },
    },
  }
}

assert.equal(normalizeEmail(' Buyer+Island@Example.COM '), 'buyer+island@example.com')
assert.equal(normalizeEmail('first.last@example.com'), 'first.last@example.com')
assert.equal(normalizePhone('+1 (787) 555-0123'), '+17875550123')
assert.throws(() => normalizePhone('787-555-0123'), /country code/)
assert.equal(normalizeDisplayName('  María   Rivera  '), 'María Rivera')
assert.equal(
  extractRecognizedPropertySlug('https://www.culebraluxe.com/properties/Casa-Luar'),
  'casa-luar',
)
assert.equal(
  extractRecognizedPropertySlug('https://example.com/properties/casa-luar'),
  null,
)

const smsPhone = normalizeIdentityHint({
  kind: 'phone',
  value: '+1 787 555 0123',
  evidence: 'provider_asserted',
})
const imessagePhone = normalizeIdentityHint({
  kind: 'phone',
  value: '+17875550123',
  evidence: 'provider_asserted',
})
assert.equal(smsPhone.normalizedValue, imessagePhone.normalizedValue)

assert.deepEqual(sanitizeRawMetadata({ threadId: 'thread-1' }), {
  threadId: 'thread-1',
})
assert.deepEqual(
  sanitizeRawMetadata({ token_count: 24, cookie_policy: 'accepted' }),
  { token_count: 24, cookie_policy: 'accepted' },
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { authorization: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { access_token: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { accessToken: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { api_key: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { apiKey: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { cookie: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ nested: { password: 'not-allowed' } }),
  /prohibited secret field/,
)
assert.throws(
  () => sanitizeRawMetadata({ payload: 'x'.repeat(33 * 1024) }),
  /32 KB limit/,
)

{
  const fake = fakeRepositories({ duplicate: interaction() })
  const result = await prepareInboundEvent(inbound(), fake.repositories)
  assert.equal(result.status, 'duplicate')
  assert.equal(result.existingInteractionId, interaction().id)
  assert.equal(fake.calls.person, 0)
  assert.equal(fake.calls.identity, 0)
  assert.equal(fake.calls.propertySlug, 0)
}

{
  const fake = fakeRepositories()
  const result = await prepareInboundEvent(inbound(), fake.repositories)
  assert.equal(result.status, 'ready')
  assert.equal(result.personResolution.personId, PERSON_A)
  assert.equal(result.propertyResolution.property?.id, PROPERTY_A)
  assert.equal(result.interactionInput?.personId, PERSON_A)
  assert.equal(result.interactionInput?.propertyId, PROPERTY_A)
  assert.equal(result.interactionInput?.sourceSystem, 'website')
  assert.equal(result.interactionInput?.sourceExternalId, 'submission-1')
  assert.equal(result.followUpIntent?.reason, 'private_viewing')
  assert.equal(result.propertyInterestIntent?.reason, 'private_viewing')
}

{
  const fake = fakeRepositories()
  const event = inbound({
    actor: {
      identityHints: [
        inbound().actor.identityHints[0],
        {
          kind: 'phone',
          value: '+1 787 555 0123',
          evidence: 'user_supplied',
        },
      ],
    },
  })
  const result = await prepareInboundEvent(event, fake.repositories)
  assert.equal(result.status, 'ready')
  assert.equal(result.personResolution.matchedIdentityIds.length, 2)
}

{
  const identities = new Map([
    [
      'email:buyer+island@example.com',
      { identityId: 'identity-a', personId: PERSON_A },
    ],
    [
      'phone:+17875550123',
      { identityId: 'identity-b', personId: PERSON_B },
    ],
  ])
  const fake = fakeRepositories({ identities })
  const result = await prepareInboundEvent(
    inbound({
      actor: {
        identityHints: [
          inbound().actor.identityHints[0],
          {
            kind: 'phone',
            value: '+17875550123',
            evidence: 'user_supplied',
          },
        ],
      },
    }),
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(result.personResolution.status, 'conflicting')
}

{
  const fake = fakeRepositories({ people: [PERSON_B] })
  const result = await prepareInboundEvent(
    inbound({ actor: { personId: PERSON_B, identityHints: inbound().actor.identityHints } }),
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(result.personResolution.status, 'conflicting')
}

{
  const fake = fakeRepositories({ identities: new Map() })
  const result = await prepareInboundEvent(
    inbound({
      actor: { identityHints: [], displayNameHint: 'María Rivera' },
      context: undefined,
    }),
    fake.repositories,
  )
  assert.equal(result.status, 'resolution_required')
}

{
  const fake = fakeRepositories()
  const result = await prepareInboundEvent(
    inbound({ context: { propertyId: PROPERTY_A } }),
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.interactionInput?.propertyId, PROPERTY_A)
}

{
  const fake = fakeRepositories()
  const result = await prepareInboundEvent(
    inbound({
      context: {
        propertyId: PROPERTY_A,
        propertySlug: 'villa-mar-azul',
      },
    }),
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(result.propertyResolution.status, 'conflicting')
}

{
  const fake = fakeRepositories()
  const result = await prepareInboundEvent(
    inbound({
      context: {
        propertyUrl: 'https://culebraluxe.com/properties/casa-luar',
      },
    }),
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.interactionInput?.propertyId, PROPERTY_A)
}

{
  const fake = fakeRepositories()
  const result = await prepareInboundEvent(
    inbound({
      content: { subject: 'Casa Luar', summary: 'Free-text mention only.' },
      context: undefined,
      intentHints: undefined,
    }),
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.propertyResolution.status, 'not_provided')
  assert.equal(result.interactionInput?.propertyId, undefined)
}

{
  const fake = fakeRepositories({ dealPersonId: PERSON_B })
  const result = await prepareInboundEvent(
    inbound({ context: { propertyId: PROPERTY_A, dealId: DEAL_A } }),
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(result.dealResolution.status, 'conflicting')
}

{
  const fake = fakeRepositories({ dealPropertyId: PROPERTY_B })
  const result = await prepareInboundEvent(
    inbound({ context: { propertyId: PROPERTY_A, dealId: DEAL_A } }),
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(result.dealResolution.status, 'conflicting')
}

{
  const externalHint = normalizeIdentityHint({
    kind: 'external',
    sourceSystem: 'HubSpot',
    value: ' contact-42 ',
    evidence: 'provider_asserted',
  })
  assert.equal(externalHint.normalizedValue, 'hubspot:contact-42')

  const identities = new Map([
    [
      'external:hubspot:contact-42',
      { identityId: 'identity-external', personId: PERSON_A },
    ],
  ])
  const fake = fakeRepositories({ identities })
  const result = await prepareInboundEvent(
    inbound({ actor: { identityHints: [externalHint] } }),
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.personResolution.personId, PERSON_A)
  assert.deepEqual(result.personResolution.matchedIdentityIds, [
    'identity-external',
  ])
}

{
  const fake = fakeRepositories()
  const result = await prepareInboundEvent(
    inbound({
      channel: 'calendar',
      eventType: 'meeting_scheduled',
      context: undefined,
      intentHints: undefined,
    }),
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.followUpIntent, undefined)
  assert.equal(result.propertyInterestIntent, undefined)
}

{
  const row = {
    id: interaction().id,
    person_id: PERSON_A,
    property_id: PROPERTY_A,
    deal_id: null,
    channel: 'website',
    event_type: 'lead_submitted',
    direction: 'inbound',
    occurred_at: '2026-08-18T12:00:00.000Z',
    title: null,
    summary: null,
    duration_seconds: null,
    source_system: 'website',
    source_external_id: 'submission-1',
    source_metadata: {},
    created_at: '2026-08-18T12:00:01.000Z',
  }
  const calls = []
  const execute = async (strings, ...parameters) => {
    calls.push({ sql: strings.join('?'), parameters })
    return [row]
  }
  const found = await getInteractionBySourceIdentity(
    ' website ',
    ' submission-1 ',
    execute,
  )
  assert.equal(found?.id, row.id)
  assert.match(calls[0].sql, /where source_system =/i)
  assert.doesNotMatch(calls[0].sql, /insert|update|delete/i)

  await assert.rejects(
    getInteractionBySourceIdentity(' ', 'submission-1', execute),
    /provided together/,
  )
  await assert.rejects(
    getInteractionBySourceIdentity('website', ' ', execute),
    /provided together/,
  )
  assert.equal(calls.length, 1)
}

console.log('CRM intake verification passed with read-only fake repositories.')

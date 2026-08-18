import assert from 'node:assert/strict'

const { normalizeInboundEvent } = await import(
  '../lib/crm-intake-normalization.ts'
)
const { resolveOrCreateInboundPerson } = await import(
  '../lib/crm-person-creation.ts'
)
const { createPersonWithIdentities } = await import(
  '../db/person-identities.ts'
)

const PERSON_A = '10000000-0000-4000-8000-000000000001'
const PERSON_B = '10000000-0000-4000-8000-000000000002'
const NEW_PERSON = '10000000-0000-4000-8000-000000000003'

const trustedEmail = (value = 'buyer@example.com') => ({
  kind: 'email',
  value,
  evidence: 'authenticated',
})
const trustedPhone = (value = '+17875550123') => ({
  kind: 'phone',
  value,
  evidence: 'provider_asserted',
})
const policy = { allowCreation: true, role: 'buyer' }

function inbound(overrides = {}) {
  return normalizeInboundEvent({
    source: { system: 'website', externalId: 'submission-1' },
    occurredAt: '2026-08-18T12:00:00.000Z',
    channel: 'website',
    eventType: 'lead_submitted',
    direction: 'inbound',
    actor: {
      identityHints: [trustedEmail()],
      displayNameHint: 'María Rivera',
      roleHint: 'seller',
    },
    rawMetadata: {},
    ...overrides,
  })
}

function ownership(personId, hint, archived = false) {
  return {
    identityId: `identity-${hint.normalizedValue}`,
    personId,
    kind: hint.kind,
    normalizedValue: hint.normalizedValue,
    archived,
  }
}

function fakeRepositories(options = {}) {
  const owned = options.owned ?? new Map()
  const calls = {
    duplicate: 0,
    person: 0,
    match: 0,
    ownership: 0,
    create: 0,
    createInputs: [],
    matchedValues: [],
  }

  return {
    calls,
    owned,
    repositories: {
      async findInteractionBySourceIdentity() {
        calls.duplicate += 1
        return options.duplicate ?? null
      },
      async personExists(personId) {
        calls.person += 1
        return (options.people ?? []).includes(personId)
      },
      async findIdentityMatch(hint) {
        calls.match += 1
        calls.matchedValues.push(hint.normalizedValue)
        const value = owned.get(`${hint.kind}:${hint.normalizedValue}`)
        return value && !value.archived ? value : null
      },
      async findIdentityOwnership(hint) {
        calls.ownership += 1
        return owned.get(`${hint.kind}:${hint.normalizedValue}`) ?? null
      },
      async createPersonWithIdentities(input) {
        calls.create += 1
        calls.createInputs.push(input)
        if (options.onCreate) await options.onCreate(input, owned)
      },
    },
  }
}

async function prepare(event, fake, selectedPolicy = policy) {
  return resolveOrCreateInboundPerson(
    event,
    selectedPolicy,
    fake.repositories,
    () => NEW_PERSON,
  )
}

{
  const fake = fakeRepositories()
  const result = await prepare(inbound(), fake)
  assert.equal(result.status, 'created')
  assert.equal(result.personId, NEW_PERSON)
  assert.equal(result.displayName, 'María Rivera')
  assert.equal(result.displayNameSource, 'hint')
  assert.equal(fake.calls.create, 1)
  assert.equal(fake.calls.createInputs[0].role, 'buyer')
  assert.equal(fake.calls.createInputs[0].identities[0].kind, 'email')
  assert.equal(fake.calls.createInputs[0].identities[0].isPrimary, true)
  assert.equal(fake.calls.matchedValues.includes('María Rivera'), false)
}

{
  const fake = fakeRepositories()
  const result = await prepare(inbound(), fake, {
    allowCreation: false,
    role: 'buyer',
  })
  assert.equal(result.status, 'resolution_required')
  assert.equal(result.reason, 'creation_not_allowed')
  assert.equal(fake.calls.create, 0)
}

{
  const fake = fakeRepositories()
  const result = await prepare(
    inbound({ actor: { identityHints: [trustedEmail()] } }),
    fake,
  )
  assert.equal(result.status, 'created')
  assert.equal(result.displayName, 'buyer@example.com')
  assert.equal(result.displayNameSource, 'email')
  assert.equal(result.claimedIdentities[0].isPrimary, true)
}

{
  const fake = fakeRepositories()
  const result = await prepare(
    inbound({
      actor: { identityHints: [trustedPhone()], roleHint: 'seller' },
    }),
    fake,
  )
  assert.equal(result.status, 'created')
  assert.equal(result.displayName, '+17875550123')
  assert.equal(result.displayNameSource, 'phone')
  assert.deepEqual(
    result.claimedIdentities.map(({ kind, isPrimary }) => ({ kind, isPrimary })),
    [{ kind: 'phone', isPrimary: true }],
  )
}

{
  const fake = fakeRepositories()
  const result = await prepare(
    inbound({
      actor: {
        identityHints: [
          trustedEmail('second@example.com'),
          trustedPhone('+17875550124'),
          trustedEmail('first-after-normalization@example.com'),
          trustedPhone('+17875550125'),
        ],
      },
    }),
    fake,
  )
  assert.equal(result.status, 'created')
  assert.deepEqual(
    result.claimedIdentities.map(({ normalizedValue, isPrimary }) => ({
      normalizedValue,
      isPrimary,
    })),
    [
      { normalizedValue: 'second@example.com', isPrimary: true },
      { normalizedValue: '+17875550124', isPrimary: true },
      {
        normalizedValue: 'first-after-normalization@example.com',
        isPrimary: false,
      },
      { normalizedValue: '+17875550125', isPrimary: false },
    ],
  )
}

{
  const fake = fakeRepositories()
  const result = await prepare(
    inbound({
      actor: {
        identityHints: [trustedEmail(), trustedEmail(' BUYER@example.com ')],
      },
    }),
    fake,
  )
  assert.equal(result.status, 'created')
  assert.equal(result.claimedIdentities.length, 1)
  assert.equal(fake.calls.match, 1)
}

for (const event of [
  inbound({
    actor: {
      identityHints: [
        { kind: 'email', value: 'buyer@example.com', evidence: 'user_supplied' },
      ],
    },
  }),
  inbound({
    actor: {
      identityHints: [
        {
          kind: 'external',
          sourceSystem: 'hubspot',
          value: 'contact-42',
          evidence: 'provider_asserted',
        },
      ],
    },
  }),
  inbound({ actor: { identityHints: [], displayNameHint: 'Name Only' } }),
  inbound({ actor: { identityHints: [] } }),
]) {
  const fake = fakeRepositories()
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolution_required')
  assert.equal(result.reason, 'insufficient_identity_evidence')
  assert.equal(fake.calls.create, 0)
}

{
  const fake = fakeRepositories()
  const result = await prepare(
    inbound({
      actor: {
        identityHints: [
          trustedEmail(),
          {
            kind: 'external',
            sourceSystem: 'HubSpot',
            value: 'contact-42',
            evidence: 'provider_asserted',
          },
        ],
      },
    }),
    fake,
  )
  assert.equal(result.status, 'created')
  assert.deepEqual(
    result.claimedIdentities.map(({ kind, normalizedValue, isPrimary }) => ({
      kind,
      normalizedValue,
      isPrimary,
    })),
    [
      { kind: 'email', normalizedValue: 'buyer@example.com', isPrimary: true },
      {
        kind: 'external',
        normalizedValue: 'hubspot:contact-42',
        isPrimary: false,
      },
    ],
  )
}

{
  const event = inbound()
  const owned = new Map([
    [
      'email:buyer@example.com',
      ownership(PERSON_A, event.actor.identityHints[0]),
    ],
  ])
  const fake = fakeRepositories({ owned })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolved_existing')
  assert.equal(result.personId, PERSON_A)
  assert.equal(fake.calls.create, 0)
}

{
  const event = inbound({
    actor: { identityHints: [trustedEmail(), trustedPhone()] },
  })
  const owned = new Map(
    event.actor.identityHints.map((hint) => [
      `${hint.kind}:${hint.normalizedValue}`,
      ownership(PERSON_A, hint),
    ]),
  )
  const fake = fakeRepositories({ owned })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolved_existing')
  assert.equal(result.personId, PERSON_A)
  assert.equal(result.unclaimedIdentities.length, 0)
}

{
  const event = inbound({
    actor: { identityHints: [trustedEmail(), trustedPhone()] },
  })
  const owned = new Map([
    [
      'email:buyer@example.com',
      ownership(PERSON_A, event.actor.identityHints[0]),
    ],
  ])
  const fake = fakeRepositories({ owned })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolved_existing')
  assert.equal(result.personId, PERSON_A)
  assert.deepEqual(
    result.unclaimedIdentities.map((hint) => hint.normalizedValue),
    ['+17875550123'],
  )
  assert.equal(fake.calls.create, 0)
}

{
  const event = inbound({
    actor: { identityHints: [trustedEmail(), trustedPhone()] },
  })
  const owned = new Map([
    [
      'email:buyer@example.com',
      ownership(PERSON_A, event.actor.identityHints[0]),
    ],
    [
      'phone:+17875550123',
      ownership(PERSON_B, event.actor.identityHints[1]),
    ],
  ])
  const fake = fakeRepositories({ owned })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'conflicting')
  assert.equal(fake.calls.create, 0)
}

{
  const fake = fakeRepositories({ people: [] })
  const result = await prepare(
    inbound({
      actor: { personId: PERSON_A, identityHints: [trustedEmail()] },
    }),
    fake,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(result.reason, 'explicit_person_not_found')
  assert.equal(fake.calls.create, 0)
}

{
  const event = inbound()
  const owned = new Map([
    [
      'email:buyer@example.com',
      ownership(PERSON_A, event.actor.identityHints[0], true),
    ],
  ])
  const fake = fakeRepositories({ owned })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolution_required')
  assert.equal(result.reason, 'archived_identity_owner')
  assert.equal(fake.calls.create, 0)
}

{
  const duplicate = {
    id: 'interaction-1',
    personId: PERSON_A,
    channel: 'website',
    eventType: 'lead_submitted',
    occurredAt: '2026-08-18T12:00:00.000Z',
    sourceSystem: 'website',
    sourceExternalId: 'submission-1',
    sourceMetadata: {},
    createdAt: '2026-08-18T12:00:01.000Z',
  }
  const fake = fakeRepositories({ duplicate })
  const result = await prepare(inbound(), fake)
  assert.equal(result.status, 'duplicate')
  assert.equal(result.personId, PERSON_A)
  assert.equal(fake.calls.person, 0)
  assert.equal(fake.calls.match, 0)
  assert.equal(fake.calls.ownership, 0)
  assert.equal(fake.calls.create, 0)
}

{
  const event = inbound()
  const uniqueViolation = Object.assign(new Error('identity race'), {
    code: '23505',
  })
  const fake = fakeRepositories({
    async onCreate(_input, owned) {
      owned.set(
        'email:buyer@example.com',
        ownership(PERSON_A, event.actor.identityHints[0]),
      )
      throw uniqueViolation
    },
  })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolved_existing')
  assert.equal(result.personId, PERSON_A)
  assert.equal(fake.calls.create, 1)
}

{
  const event = inbound({
    actor: { identityHints: [trustedEmail(), trustedPhone()] },
  })
  const uniqueViolation = Object.assign(new Error('identity race'), {
    code: '23505',
  })
  const fake = fakeRepositories({
    async onCreate(_input, owned) {
      owned.set(
        'email:buyer@example.com',
        ownership(PERSON_A, event.actor.identityHints[0]),
      )
      throw uniqueViolation
    },
  })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'resolved_existing')
  assert.equal(result.personId, PERSON_A)
  assert.deepEqual(
    result.unclaimedIdentities.map((hint) => hint.normalizedValue),
    ['+17875550123'],
  )
  assert.equal(fake.calls.create, 1)
}

{
  const event = inbound({
    actor: { identityHints: [trustedEmail(), trustedPhone()] },
  })
  const uniqueViolation = Object.assign(new Error('identity race'), {
    code: '23505',
  })
  const fake = fakeRepositories({
    async onCreate(_input, owned) {
      owned.set(
        'email:buyer@example.com',
        ownership(PERSON_A, event.actor.identityHints[0]),
      )
      owned.set(
        'phone:+17875550123',
        ownership(PERSON_B, event.actor.identityHints[1]),
      )
      throw uniqueViolation
    },
  })
  const result = await prepare(event, fake)
  assert.equal(result.status, 'conflicting')
  assert.equal(fake.calls.create, 1)
}

{
  const fake = fakeRepositories({
    async onCreate() {
      throw new Error('database unavailable')
    },
  })
  const result = await prepare(inbound(), fake)
  assert.equal(result.status, 'rejected')
  assert.equal(result.reason, 'repository_failure')
  assert.equal(fake.calls.create, 1)
}

{
  const calls = []
  let transactionCount = 0
  const executeTransaction = async (buildQueries) => {
    transactionCount += 1
    const execute = async (strings, ...parameters) => {
      calls.push({ sql: strings.join('?'), parameters })
      return []
    }
    return Promise.all(buildQueries(execute))
  }

  await createPersonWithIdentities(
    {
      personId: NEW_PERSON,
      displayName: 'María Rivera',
      role: 'buyer',
      identities: [
        {
          kind: 'phone',
          normalizedValue: '+17875550123',
          isPrimary: true,
        },
        {
          kind: 'email',
          normalizedValue: 'buyer@example.com',
          isPrimary: true,
        },
      ],
    },
    executeTransaction,
  )

  assert.equal(transactionCount, 1)
  assert.equal(calls.length, 3)
  assert.match(calls[0].sql, /insert into person/i)
  assert.match(calls[1].sql, /insert into person_identity/i)
  assert.equal(calls[1].parameters[2], 'buyer@example.com')
  assert.equal(calls[1].parameters[4], true)
  assert.equal(calls[2].parameters[2], '+17875550123')
  assert.equal(calls[2].parameters[4], true)
  assert.doesNotMatch(calls.map((call) => call.sql).join('\n'), /on conflict/i)
}

console.log('CRM person creation verification passed without Neon access.')

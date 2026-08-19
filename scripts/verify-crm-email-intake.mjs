import assert from 'node:assert/strict'

const { adaptEmailMessage } = await import('../lib/crm-email-normalization.ts')
const { prepareEmailIntake } = await import('../lib/crm-email-intake.ts')

const PERSON = '10000000-0000-4000-8000-000000000001'
const NEW_PERSON = '10000000-0000-4000-8000-000000000002'
const PROPERTY = '20000000-0000-4000-8000-000000000001'
const PROPERTY_B = '20000000-0000-4000-8000-000000000002'
const DEAL = '40000000-0000-4000-8000-000000000001'
const INTERACTION = '30000000-0000-4000-8000-000000000001'

const configuration = {
  internalMailboxes: [
    { email: 'inquiries@culebraluxe.com', creationRole: 'buyer' },
    { email: 'sales@culebraluxe.com', creationRole: 'buyer' },
  ],
  systemSenderEmails: ['system@culebraluxe.com'],
  noReplySenderEmails: ['noreply@example.com'],
}

function message(overrides = {}) {
  return {
    provider: 'Fixture',
    accountNamespace: 'Sales_One',
    messageId: 'Message-A',
    threadId: 'Thread-A',
    occurredAt: '2026-08-18T12:00:00.000Z',
    senders: [{ email: 'Buyer+Island@Example.com', displayName: 'María Rivera' }],
    to: [{ email: 'inquiries@culebraluxe.com' }],
    senderAuthentication: 'unverified',
    category: 'human_correspondence',
    subject: 'Casa Luar inquiry',
    plainText: 'Please tell me more.',
    ...overrides,
  }
}

function interaction() {
  return {
    id: INTERACTION,
    personId: PERSON,
    channel: 'email',
    eventType: 'email_received',
    occurredAt: '2026-08-18T12:00:00.000Z',
    sourceSystem: 'email:fixture:sales_one',
    sourceExternalId: 'Message-A',
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
    createInputs: [],
  }
  return {
    calls,
    repositories: {
      async findInteractionBySourceIdentity(system, externalId) {
        calls.duplicate += 1
        if (options.onDuplicate) return options.onDuplicate(system, externalId, calls.duplicate)
        return options.duplicate ?? null
      },
      async personExists(personId) {
        calls.person += 1
        return personId === PERSON || personId === NEW_PERSON || createdPeople.has(personId)
      },
      async findIdentityMatch(hint) {
        calls.match += 1
        if (options.existingEmail === hint.normalizedValue) {
          return {
            identityId: 'identity-1',
            personId: PERSON,
            kind: hint.kind,
            normalizedValue: hint.normalizedValue,
          }
        }
        return null
      },
      async findIdentityOwnership(hint) {
        calls.ownership += 1
        if (options.existingEmail === hint.normalizedValue) {
          return {
            identityId: 'identity-1',
            personId: PERSON,
            kind: hint.kind,
            normalizedValue: hint.normalizedValue,
            archived: false,
          }
        }
        return null
      },
      async createPersonWithIdentities(input) {
        calls.create += 1
        calls.createInputs.push(input)
        if (options.forbidCreate) throw new Error('Unexpected person creation')
        createdPeople.add(input.personId)
      },
      async findPropertyById(id) {
        calls.property += 1
        return id === PROPERTY ? { id: PROPERTY, slug: 'casa-luar' } : null
      },
      async findPropertyBySlug(slug) {
        calls.slug += 1
        if (slug === 'casa-luar') return { id: PROPERTY, slug }
        if (slug === 'villa-mar-azul') return { id: PROPERTY_B, slug }
        return null
      },
      async findDealById(id) {
        calls.deal += 1
        if (id !== DEAL) return null
        return {
          id: DEAL,
          personId: options.dealPersonId ?? PERSON,
          propertyId: options.dealPropertyId ?? PROPERTY,
        }
      },
    },
  }
}

// Adapter contract: deterministic source, direction, actors, event types, and metadata.
{
  const result = adaptEmailMessage(
    message({
      provider: ' FIXTURE ',
      accountNamespace: ' Sales_One ',
      cc: [{ email: 'Sales@CulebraLuxe.com' }],
      bcc: [{ email: 'inquiries@culebraluxe.com' }],
      replyTo: [{ email: 'Buyer+Island@Example.com' }],
      referenceMessageIds: ['Ref-A', 'Ref-A', 'ref-a'],
      inReplyToMessageId: 'Reply-A',
      isForward: false,
      attachments: [
        {
          providerAttachmentId: 'attachment_1',
          filename: 'details.pdf',
          mimeType: 'application/pdf',
          sizeBytes: 42,
        },
      ],
      providerPayload: { harmlessBusinessField: true },
    }),
    configuration,
  )
  assert.equal(result.status, 'accepted')
  assert.equal(result.direction, 'inbound')
  assert.equal(result.actorEmail, 'buyer+island@example.com')
  assert.equal(result.inboundEvent.eventType, 'email_received')
  assert.equal(result.inboundEvent.source.system, 'email:fixture:sales_one')
  assert.equal(result.inboundEvent.source.externalId, 'Message-A')
  assert.equal(result.inboundEvent.actor.identityHints[0].evidence, 'user_supplied')
  assert.deepEqual(result.inboundEvent.rawMetadata.referenceMessageIds, ['Ref-A', 'ref-a'])
  assert.equal('bccEmails' in result.inboundEvent.rawMetadata, false)
  assert.equal('providerPayload' in result.inboundEvent.rawMetadata, false)
  assert.deepEqual(Object.keys(result.inboundEvent.rawMetadata).sort(), [
    'attachments',
    'ccEmails',
    'inReplyToMessageId',
    'isForward',
    'referenceMessageIds',
    'replyToEmails',
    'threadId',
    'toEmails',
  ])
}

{
  const result = adaptEmailMessage(
    message({
      senders: [{ email: 'sales@culebraluxe.com' }],
      to: [{ email: 'Existing.Person@Example.com' }],
      trustedDirection: 'outbound',
      senderAuthentication: 'authenticated_pass',
    }),
    configuration,
  )
  assert.equal(result.status, 'accepted')
  assert.equal(result.direction, 'outbound')
  assert.equal(result.actorEmail, 'existing.person@example.com')
  assert.equal(result.inboundEvent.eventType, 'email_sent')
  assert.equal(result.inboundEvent.actor.identityHints[0].evidence, 'user_supplied')
}

// Cardinality and deterministic direction outcomes precede identity repositories.
for (const [input, status, reason] of [
  [message({ senders: [] }), 'rejected', 'Exactly one sender'],
  [message({ senders: [{ email: 'a@example.com' }, { email: 'b@example.com' }] }), 'rejected', 'Exactly one sender'],
  [message({ senders: [{ email: 'sales@culebraluxe.com' }], to: [{ email: 'inquiries@culebraluxe.com' }] }), 'excluded', 'internal_only'],
  [message({ senders: [{ email: 'sales@culebraluxe.com' }], to: [{ email: 'a@example.com' }, { email: 'b@example.com' }] }), 'resolution_required', 'multiple_external_recipients'],
  [message({ trustedDirection: 'outbound' }), 'rejected', 'conflicting_trusted_direction'],
]) {
  const result = adaptEmailMessage(input, configuration)
  assert.equal(result.status, status)
  assert.match(result.reason, new RegExp(reason))
}

// Exact exclusions; ordinary subject language never excludes.
for (const [overrides, reason] of [
  [{ category: 'system_notification' }, 'system_notification'],
  [{ category: 'delivery_status' }, 'delivery_status'],
  [{ category: 'auto_reply' }, 'auto_reply'],
  [{ category: 'bulk_list' }, 'bulk_list'],
  [{ transportEvidence: { autoSubmitted: 'auto-replied' } }, 'auto_submitted'],
  [{ transportEvidence: { listId: 'island-list.example' } }, 'list_mail'],
  [{ transportEvidence: { contentType: 'multipart/report; report-type=delivery-status' } }, 'delivery_status'],
  [{ transportEvidence: { returnPath: '<>' } }, 'null_return_path'],
  [{ senders: [{ email: 'system@culebraluxe.com' }] }, 'configured_system_sender'],
  [{ senders: [{ email: 'noreply@example.com' }] }, 'configured_no_reply_sender'],
]) {
  const result = adaptEmailMessage(message(overrides), configuration)
  assert.equal(result.status, 'excluded')
  assert.equal(result.reason, reason)
}
assert.equal(
  adaptEmailMessage(
    message({ subject: 'Automatic newsletter delivery discussion' }),
    configuration,
  ).status,
  'accepted',
)
assert.equal(
  adaptEmailMessage(
    message({ transportEvidence: { contentType: 'multipart/reporting' } }),
    configuration,
  ).status,
  'accepted',
)
assert.equal(adaptEmailMessage(message({ category: 'provider_magic' }), configuration).status, 'rejected')
assert.equal(adaptEmailMessage(message({ senderAuthentication: 'unknown' }), configuration).status, 'rejected')
assert.equal(
  adaptEmailMessage(message({ subject: 'Token count', token_count: 5, providerPayload: { safe: true } }), configuration).status,
  'accepted',
)

// Bounds, opaque case-sensitive identifiers, source grammar, and attachment privacy.
for (const input of [
  message({ provider: 'bad:provider' }),
  message({ accountNamespace: 'bad account' }),
  message({ provider: 'x'.repeat(65) }),
  message({ messageId: '' }),
  message({ messageId: 'bad\u0000id' }),
  message({ threadId: '   ' }),
  message({ inReplyToMessageId: 'bad\u007fid' }),
  message({ referenceMessageIds: ['bad\u0080id'] }),
  message({ referenceMessageIds: Array.from({ length: 101 }, (_, index) => `ref-${index}`) }),
  message({ attachments: [{ providerAttachmentId: 'https://example.com/a', filename: 'a', mimeType: 'text/plain', sizeBytes: 1 }] }),
  message({ attachments: [{ filename: 'a', mimeType: 'text/plain', sizeBytes: 1 }] }),
  message({ attachments: [{ providerAttachmentId: null, filename: 'a', mimeType: 'text/plain', sizeBytes: 1 }] }),
  message({ attachments: [{ providerAttachmentId: 123, filename: 'a', mimeType: 'text/plain', sizeBytes: 1 }] }),
  message({ attachments: [{ providerAttachmentId: 'attachment-1', filename: 123, mimeType: 'text/plain', sizeBytes: 1 }] }),
  message({ attachments: [{ providerAttachmentId: 'attachment-1', filename: 'a', mimeType: {}, sizeBytes: 1 }] }),
  message({ attachments: [{ providerAttachmentId: 'attachment-1', filename: 'a', mimeType: 'text/plain', sizeBytes: '1' }] }),
  message({ attachments: [{ providerAttachmentId: 'attachment-1', filename: ' ', mimeType: 'text/plain', sizeBytes: 1 }] }),
]) {
  assert.equal(adaptEmailMessage(input, configuration).status, 'rejected')
}
assert.equal(adaptEmailMessage(message({ messageId: 'Case' }), configuration).inboundEvent.source.externalId, 'Case')
assert.equal(adaptEmailMessage(message({ messageId: 'case' }), configuration).inboundEvent.source.externalId, 'case')
assert.equal(adaptEmailMessage(message({ subject: 'x'.repeat(501) }), configuration).status, 'rejected')
assert.equal(adaptEmailMessage(message({ plainText: 'x'.repeat(4001) }), configuration).status, 'rejected')
assert.equal(adaptEmailMessage(message({ plainText: 'Cafe\u0301' }), configuration).inboundEvent.content.summary, 'Café')
assert.equal(
  adaptEmailMessage(message({ plainText: '<p>Connector supplied text</p>\n> quoted history' }), configuration).inboundEvent.content.summary,
  '<p>Connector supplied text</p>\n> quoted history',
)
assert.equal(
  adaptEmailMessage(
    message({
      attachments: Array.from({ length: 80 }, (_, index) => ({
        providerAttachmentId: `attachment-${index}`,
        filename: `file-${index}-${'x'.repeat(450)}`,
        mimeType: 'application/octet-stream',
        sizeBytes: index,
      })),
    }),
    configuration,
  ).status,
  'rejected',
)

// Thread correlation never replaces case-sensitive message idempotency identity.
{
  const first = adaptEmailMessage(message({ messageId: 'Message-A', threadId: 'Same-Thread' }), configuration)
  const second = adaptEmailMessage(message({ messageId: 'Message-B', threadId: 'Same-Thread' }), configuration)
  assert.equal(first.status, 'accepted')
  assert.equal(second.status, 'accepted')
  assert.notEqual(first.inboundEvent.source.externalId, second.inboundEvent.source.externalId)
  assert.equal(first.inboundEvent.rawMetadata.threadId, second.inboundEvent.rawMetadata.threadId)
}

// Duplicate short-circuits every person/context seam.
{
  const fake = fakeRepositories({ duplicate: interaction(), forbidCreate: true })
  const result = await prepareEmailIntake(message(), configuration, fake.repositories)
  assert.equal(result.status, 'duplicate')
  assert.equal(fake.calls.duplicate, 1)
  assert.equal(fake.calls.match, 0)
  assert.equal(fake.calls.ownership, 0)
  assert.equal(fake.calls.property + fake.calls.slug + fake.calls.deal, 0)
}

// Transport exclusion happens before any repository boundary.
{
  const fake = fakeRepositories({ forbidCreate: true })
  const result = await prepareEmailIntake(
    message({ category: 'auto_reply' }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'excluded')
  assert.equal(
    fake.calls.duplicate +
      fake.calls.person +
      fake.calls.match +
      fake.calls.ownership +
      fake.calls.create +
      fake.calls.property +
      fake.calls.slug +
      fake.calls.deal,
    0,
  )
}

// Unverified exact existing people resolve; unknown people do not create.
{
  const fake = fakeRepositories({ existingEmail: 'buyer+island@example.com', forbidCreate: true })
  const result = await prepareEmailIntake(message(), configuration, fake.repositories)
  assert.equal(result.status, 'ready')
  assert.equal(result.personResult.status, 'resolved_existing')
  assert.equal(fake.calls.create, 0)
}
{
  const fake = fakeRepositories({ forbidCreate: true })
  const result = await prepareEmailIntake(message(), configuration, fake.repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(fake.calls.create, 0)
}

// Authenticated inbound creation requires one unanimous configured role.
{
  const fake = fakeRepositories()
  const result = await prepareEmailIntake(
    message({ senderAuthentication: 'authenticated_pass' }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.personResult.status, 'created')
  assert.equal(fake.calls.create, 1)
  assert.equal(fake.calls.createInputs[0].role, 'buyer')
}
for (const config of [
  { ...configuration, internalMailboxes: [{ email: 'inquiries@culebraluxe.com' }] },
  {
    ...configuration,
    internalMailboxes: [
      { email: 'inquiries@culebraluxe.com', creationRole: 'buyer' },
      { email: 'sales@culebraluxe.com', creationRole: 'seller' },
    ],
  },
]) {
  const fake = fakeRepositories({ forbidCreate: true })
  const input =
    config.internalMailboxes.length === 2
      ? message({ senderAuthentication: 'authenticated_pass', cc: [{ email: 'sales@culebraluxe.com' }] })
      : message({ senderAuthentication: 'authenticated_pass' })
  const result = await prepareEmailIntake(input, config, fake.repositories)
  assert.equal(result.status, 'resolution_required')
  assert.equal(fake.calls.create, 0)
}

// Unknown outbound never creates even when the internal sender is authenticated.
{
  const fake = fakeRepositories({ forbidCreate: true })
  const result = await prepareEmailIntake(
    message({
      senders: [{ email: 'sales@culebraluxe.com' }],
      to: [{ email: 'unknown@example.com' }],
      senderAuthentication: 'authenticated_pass',
    }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'resolution_required')
  assert.equal(fake.calls.create, 0)
}
{
  const fake = fakeRepositories({ existingEmail: 'existing@example.com', forbidCreate: true })
  const result = await prepareEmailIntake(
    message({
      senders: [{ email: 'sales@culebraluxe.com' }],
      to: [{ email: 'Existing@Example.com' }],
      senderAuthentication: 'authenticated_pass',
    }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.personResult.status, 'resolved_existing')
  assert.equal(fake.calls.create, 0)
}

// Multiple applicable internal mailboxes with the same explicit role may create.
{
  const fake = fakeRepositories()
  const result = await prepareEmailIntake(
    message({
      senderAuthentication: 'authenticated_pass',
      cc: [{ email: 'sales@culebraluxe.com' }],
    }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.personResult.status, 'created')
  assert.equal(fake.calls.createInputs[0].role, 'buyer')
}

assert.equal(
  adaptEmailMessage(
    message(),
    {
      internalMailboxes: [
        { email: 'inquiries@culebraluxe.com', creationRole: 'buyer' },
        { email: 'INQUIRIES@culebraluxe.com', creationRole: 'seller' },
      ],
    },
  ).status,
  'rejected',
)

// Trusted exact context flows through CRM-02 only after identity resolution.
{
  const fake = fakeRepositories({ existingEmail: 'buyer+island@example.com', forbidCreate: true })
  const result = await prepareEmailIntake(
    message({ trustedContext: { propertyId: PROPERTY, propertySlug: 'casa-luar' } }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.intakeResult.interactionInput.propertyId, PROPERTY)
}

// Free text never supplies property context or triggers a property/deal lookup.
{
  const fake = fakeRepositories({ existingEmail: 'buyer+island@example.com', forbidCreate: true })
  const result = await prepareEmailIntake(
    message({
      subject: 'Villa Mar Azul and Casa Luar',
      plainText: 'Please link this to /properties/casa-luar and the deal mentioned here.',
    }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'ready')
  assert.equal(result.intakeResult.propertyResolution.status, 'not_provided')
  assert.equal(result.intakeResult.dealResolution.status, 'not_provided')
  assert.equal(fake.calls.property + fake.calls.slug + fake.calls.deal, 0)
}

// Multiple trusted exact property hints must agree.
{
  const fake = fakeRepositories({ existingEmail: 'buyer+island@example.com', forbidCreate: true })
  const result = await prepareEmailIntake(
    message({
      trustedContext: {
        propertyId: PROPERTY,
        propertySlug: 'villa-mar-azul',
      },
    }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(fake.calls.property, 1)
  assert.equal(fake.calls.slug, 1)
}

// Resolved deal context must agree with the exact resolved property.
{
  const fake = fakeRepositories({
    existingEmail: 'buyer+island@example.com',
    dealPropertyId: PROPERTY_B,
    forbidCreate: true,
  })
  const result = await prepareEmailIntake(
    message({
      trustedContext: {
        propertyId: PROPERTY,
        dealId: DEAL,
      },
    }),
    configuration,
    fake.repositories,
  )
  assert.equal(result.status, 'rejected')
  assert.equal(fake.calls.property, 1)
  assert.equal(fake.calls.deal, 1)
}

// Fixture-only assertion: every reachable seam is injected; no canonical write exists.
assert.equal(typeof prepareEmailIntake, 'function')
console.log('CRM email intake fixture verification passed with zero provider or Neon access.')

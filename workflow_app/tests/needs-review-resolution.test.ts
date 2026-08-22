import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { PortalWriteError } from '../../lib/portal-write-error'
import { resolveIntake } from '../../db/needs-review-resolution'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

// ---------------------------------------------------------------------------
// OPS-06 — Intake / Resolution Administration: targeted unit suite for the
// Needs Review resolution seam (db/needs-review-resolution.ts).
//
// The single resolveIntake command (attach | create | reject) is exercised
// through an injected in-memory TxRunner that models the receipt / person /
// person_identity (with the person_identity_unique backstop) / interaction
// (with the source-identity backstop) / property_interest / task tables, and
// snapshots state so a throw rolls back like the real interactive
// transaction. No database is touched (db/client.ts is lazy and never
// queried). Real-Postgres coverage of these seams lives in the persistence
// suite.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

const SUBMISSION = '40000000-0000-4000-8000-000000000020'
const PROPERTY = '40000000-0000-4000-8000-000000000001'
const PERSON_A = '10000000-0000-4000-8000-000000000001'
const PERSON_B = '10000000-0000-4000-8000-000000000002'
const ACTOR = '20000000-0000-4000-8000-000000000001'
const EXISTING_INTERACTION = '50000000-0000-4000-8000-000000000020'
const EMAIL = 'buyer@example.com'
const NAME = 'María Rivera'
const NOW = '2026-08-22T12:00:00.000Z'

function receipt(overrides: Row = {}) {
  return {
    id: SUBMISSION,
    request_type: 'private_viewing',
    property_id: PROPERTY,
    display_name: NAME,
    email: EMAIL,
    message: 'I would like to visit.',
    status: 'received',
    created_at: NOW,
    interaction_id: null,
    resolved_by_user_id: null,
    resolved_at: null,
    ...overrides,
  }
}

function person(id: string, displayName: string) {
  return {
    id,
    display_name: displayName,
    role: 'buyer',
    status: 'new',
    archived_at: null,
  }
}

function identity(personId: string, value: string, overrides: Row = {}) {
  return {
    id: `identity-${value}`,
    person_id: personId,
    identity_type: 'email',
    identity_value: value,
    source_system: null,
    is_primary: true,
    ...overrides,
  }
}

function baseState(overrides: Row = {}) {
  return {
    receipts: new Map([[SUBMISSION, receipt()]]),
    persons: new Map([
      [PERSON_A, person(PERSON_A, 'Ana Torres')],
      [PERSON_B, person(PERSON_B, 'Bruno Cruz')],
    ]),
    identities: new Map<string, Row>(),
    interactions: new Map<string, Row>(),
    tasks: new Set<string>(),
    interests: new Set<string>(),
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// In-memory transaction fake. Dispatches the seam's own SQL by stable
// fragments and mutates the shared state so assertions inspect real
// post-conditions (not just call counts). The runner snapshots state and
// rolls back on error, mirroring the real interactive transaction boundary.
// ---------------------------------------------------------------------------

function makeTx(state: Row, options: Row = {}) {
  const statements: { text: string; values: unknown[] }[] = []

  const execute: QueryExecutor = async (strings, ...values) => {
    const text = strings.join('?').replace(/\s+/g, ' ').trim()
    statements.push({ text, values })

    // Receipt load (locked): select ... for update
    if (text.includes('from website_intake_submission') && text.includes('for update')) {
      const row = state.receipts.get(values[0])
      return row ? [row] : []
    }

    // Receipt terminal update (complete | reject)
    if (text.startsWith('update website_intake_submission')) {
      const isComplete = text.includes("status = 'completed'")
      // complete: values = [interactionId, actorAppUserId, submissionId]
      // reject:   values = [actorAppUserId, submissionId]
      const submissionId = isComplete ? values[2] : values[1]
      const row = state.receipts.get(submissionId)
      if (!row || !['received', 'resolution_required'].includes(row.status)) {
        return []
      }
      row.status = isComplete ? 'completed' : 'rejected'
      row.processing_started_at = null
      row.interaction_id = isComplete ? values[0] : null
      row.resolved_by_user_id = values[isComplete ? 1 : 0]
      row.resolved_at = NOW
      row.updated_at = NOW
      return [{ id: row.id }]
    }

    // personExists: select id from person where id = ... and archived_at is null
    if (text.includes('select id from person') && text.includes('archived_at is null')) {
      const row = state.persons.get(values[0])
      return row && row.archived_at === null ? [{ id: row.id }] : []
    }

    // findIdentityOwnership: from person_identity pi join person p
    if (text.includes('from person_identity pi')) {
      const identityValue = values[1]
      const owner = state.identities.get(`email:${identityValue}`)
      if (!owner) return []
      const ownerPerson = state.persons.get(owner.person_id)
      return [
        {
          identity_id: owner.id,
          person_id: owner.person_id,
          identity_value: owner.identity_value,
          archived_at: ownerPerson ? ownerPerson.archived_at : null,
        },
      ]
    }

    // personDisplayName: select display_name from person where id = ...
    if (text.includes('select display_name from person')) {
      const row = state.persons.get(values[0])
      return row ? [{ display_name: row.display_name }] : []
    }

    // createPersonWithIdentities: insert into person (...) values (...)
    // values = [personId, displayName, role]; status 'new' is a SQL literal.
    if (text.startsWith('insert into person (')) {
      const [id, displayName, role] = values
      state.persons.set(id, {
        id,
        display_name: displayName,
        role,
        status: 'new',
        archived_at: null,
      })
      return []
    }

    // person_identity insert — create path (values) or attach path (select).
    if (text.startsWith('insert into person_identity (')) {
      const isSelectShape = text.includes('select ') && text.includes('where not exists')
      if (!isSelectShape) {
        // create path: values = [personId, type, value, sourceSystem, isPrimary]
        const [personId, type, value, sourceSystem, isPrimary] = values
        if (state.identities.has(`${type}:${value}`)) {
          // person_identity_unique backstop (Postgres error 23505).
          const error = new Error(
            'duplicate key value violates unique constraint "person_identity_unique"',
          )
          error.code = '23505'
          throw error
        }
        if (options.failIdentityInsertFor === `${type}:${value}`) {
          // Simulated concurrent claim between ownership check and insert.
          const error = new Error(
            'duplicate key value violates unique constraint "person_identity_unique"',
          )
          error.code = '23505'
          throw error
        }
        state.identities.set(`${type}:${value}`, {
          id: `identity-${value}`,
          person_id: personId,
          identity_type: type,
          identity_value: value,
          source_system: sourceSystem ?? null,
          is_primary: isPrimary,
        })
        return []
      }
      // attach path: values = [personId, emailValue, ...]
      // (the type literal and is_primary expression are SQL, not parameters)
      const [personId, emailValue] = values
      const type = 'email'
      const alreadyForPerson = [...state.identities.values()].some(
        (i) =>
          i.person_id === personId &&
          i.identity_type === type &&
          i.identity_value === emailValue,
      )
      const alreadyOwned = state.identities.has(`${type}:${emailValue}`)
      if (!alreadyForPerson && !alreadyOwned) {
        const hasEmailForPerson = [...state.identities.values()].some(
          (i) => i.person_id === personId && i.identity_type === 'email',
        )
        state.identities.set(`${type}:${emailValue}`, {
          id: `identity-${emailValue}`,
          person_id: personId,
          identity_type: type,
          identity_value: emailValue,
          source_system: null,
          is_primary: !hasEmailForPerson,
        })
      }
      return []
    }

    // Canonical interaction insert (persistCanonicalWebsiteIntake).
    // values = [interactionId, personId, propertyId, eventType, occurredAt,
    //           title, message, submissionId, metadata]
    if (text.startsWith('insert into interaction (')) {
      const [id, personId, propertyId, , occurredAt, title, , submissionId] = values
      const key = `website:${submissionId}`
      if (state.interactions.has(key)) return [] // on conflict ... do nothing
      state.interactions.set(key, {
        id,
        person_id: personId,
        property_id: propertyId ?? null,
        source_system: 'website',
        source_external_id: submissionId,
        occurred_at: occurredAt,
        title,
        summary: null,
      })
      return [{ id }]
    }

    // Interaction source-identity lookup (dedupe resolution).
    // values = [submissionId]; source_system is the literal 'website'.
    if (text.includes('select id from interaction') && text.includes('source_external_id')) {
      const row = state.interactions.get(`website:${values[0]}`)
      return row ? [{ id: row.id }] : []
    }

    // property_interest insert (only for property-scoped intakes).
    if (text.startsWith('insert into property_interest (')) {
      const [personId, propertyId] = values
      const key = `${personId}:${propertyId}`
      if (state.interests.has(key)) return []
      state.interests.add(key)
      return [{ id: 'interest-1' }]
    }

    // task insert: values = [title, detail, personId, propertyId, sourceInteractionId, kind, priority]
    if (text.startsWith('insert into task (')) {
      const [, , , , sourceInteractionId] = values
      const interactionExists = [...state.interactions.values()].some(
        (i) => i.id === sourceInteractionId,
      )
      if (!interactionExists) return []
      state.tasks.add(sourceInteractionId)
      return [{ id: 'task-1' }]
    }

    throw new Error(`needs-review fake: unhandled statement — ${text.slice(0, 120)}`)
  }

  return { execute, statements }
}

function snapshotState(state: Row) {
  return {
    receipts: new Map([...state.receipts].map(([k, v]) => [k, structuredClone(v)])),
    persons: new Map([...state.persons].map(([k, v]) => [k, structuredClone(v)])),
    identities: new Map([...state.identities].map(([k, v]) => [k, structuredClone(v)])),
    interactions: new Map([...state.interactions].map(([k, v]) => [k, structuredClone(v)])),
    tasks: new Set(state.tasks),
    interests: new Set(state.interests),
  }
}

function restoreState(state: Row, snapshot: Row) {
  state.receipts = snapshot.receipts
  state.persons = snapshot.persons
  state.identities = snapshot.identities
  state.interactions = snapshot.interactions
  state.tasks = snapshot.tasks
  state.interests = snapshot.interests
}

function fakeRunner(state: Row, options: Row = {}): TxRunner & { statements: Row[] } {
  const tx = makeTx(state, options)
  return {
    statements: tx.statements,
    async run(cb) {
      const snapshot = snapshotState(state)
      try {
        return await cb(tx.execute)
      } catch (error) {
        restoreState(state, snapshot)
        throw error
      }
    },
  }
}

function isConflict(error: unknown) {
  return error instanceof PortalWriteError && error.code === 'conflict'
}

// ---------------------------------------------------------------------------
// 1. attach — links to the selected person, records the email identity,
//    persists the canonical interaction, completes the receipt, captures actor.
// ---------------------------------------------------------------------------
test('attach: links the intake to the explicitly selected person and completes', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  const result = await resolveIntake(
    {
      submissionId: SUBMISSION,
      action: { kind: 'attach', personId: PERSON_A },
      actorAppUserId: ACTOR,
    },
    runner.run,
  )
  assert.equal(result.status, 'completed')
  assert.equal(result.personId, PERSON_A)
  assert.equal(typeof result.interactionId, 'string')
  assert.equal(state.receipts.get(SUBMISSION).status, 'completed')
  assert.equal(state.receipts.get(SUBMISSION).interaction_id, result.interactionId)
  assert.equal(state.receipts.get(SUBMISSION).resolved_by_user_id, ACTOR)
  assert.equal(state.receipts.get(SUBMISSION).resolved_at, NOW)
  assert.equal(state.interactions.size, 1)
  assert.equal([...state.interactions.values()][0].person_id, PERSON_A)
  assert.equal(state.tasks.size, 1)
  const attachedIdentity = state.identities.get(`email:${EMAIL}`)
  assert.equal(attachedIdentity.person_id, PERSON_A)
  assert.equal(attachedIdentity.is_primary, true)
  assert.equal(state.persons.size, 2)
})

// ---------------------------------------------------------------------------
// 2. create — operator-authorized person from the intake identity
//    (user_supplied email, role buyer, display name from the intake), canonical
//    interaction, completed receipt, actor capture.
// ---------------------------------------------------------------------------
test('create: operator-authorized person creation completes the intake', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  const result = await resolveIntake(
    { submissionId: SUBMISSION, action: { kind: 'create' }, actorAppUserId: ACTOR },
    runner.run,
  )
  assert.equal(result.status, 'completed')
  const created = state.persons.get(result.personId)
  assert.equal(created.display_name, NAME)
  assert.equal(created.role, 'buyer')
  assert.equal(created.status, 'new')
  const createdIdentity = state.identities.get(`email:${EMAIL}`)
  assert.equal(createdIdentity.person_id, result.personId)
  assert.equal(createdIdentity.is_primary, true)
  assert.equal(state.receipts.get(SUBMISSION).status, 'completed')
  assert.equal(state.receipts.get(SUBMISSION).interaction_id, result.interactionId)
  assert.equal(state.receipts.get(SUBMISSION).resolved_by_user_id, ACTOR)
  assert.equal(state.receipts.get(SUBMISSION).resolved_at, NOW)
  assert.equal(state.interactions.size, 1)
  assert.equal(state.tasks.size, 1)
})

// ---------------------------------------------------------------------------
// 3. attach conflict — intake email owned by a different person: clear
//    conflict refusal, zero writes (never silently reassign).
// ---------------------------------------------------------------------------
test('attach: refuses when the intake email is owned by a different person', async () => {
  const state = baseState({
    identities: new Map([[`email:${EMAIL}`, identity(PERSON_B, EMAIL)]]),
  })
  const runner = fakeRunner(state)
  await assert.rejects(
    resolveIntake(
      { submissionId: SUBMISSION, action: { kind: 'attach', personId: PERSON_A } },
      runner.run,
    ),
    (error) =>
      isConflict(error) &&
      /Bruno Cruz/.test(error.message) &&
      /Attach to that person instead/.test(error.message),
  )
  assert.equal(state.receipts.get(SUBMISSION).status, 'received')
  assert.equal(state.interactions.size, 0)
  assert.equal(state.tasks.size, 0)
  assert.equal(state.identities.has(`email:${EMAIL}`), true)
  assert.equal(state.identities.get(`email:${EMAIL}`).person_id, PERSON_B)
})

// ---------------------------------------------------------------------------
// 4. create conflict — intake email already owned: refuse to create a second
//    owner (person_identity_unique backstop would block it anyway).
// ---------------------------------------------------------------------------
test('create: refuses a second owner when the email is already owned', async () => {
  const state = baseState({
    identities: new Map([[`email:${EMAIL}`, identity(PERSON_A, EMAIL)]]),
  })
  const runner = fakeRunner(state)
  await assert.rejects(
    resolveIntake({ submissionId: SUBMISSION, action: { kind: 'create' } }, runner.run),
    (error) => isConflict(error) && /Ana Torres/.test(error.message),
  )
  assert.equal(state.persons.size, 2)
  assert.equal(state.receipts.get(SUBMISSION).status, 'received')
  assert.equal(state.interactions.size, 0)
})

// ---------------------------------------------------------------------------
// 5. reject — transitions the receipt to rejected and captures the actor.
// ---------------------------------------------------------------------------
test('reject: transitions the receipt to rejected with actor capture', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  const result = await resolveIntake(
    { submissionId: SUBMISSION, action: { kind: 'reject' }, actorAppUserId: ACTOR },
    runner.run,
  )
  assert.deepEqual(result, { submissionId: SUBMISSION, status: 'rejected' })
  assert.equal(state.receipts.get(SUBMISSION).status, 'rejected')
  assert.equal(state.receipts.get(SUBMISSION).interaction_id, null)
  assert.equal(state.receipts.get(SUBMISSION).resolved_by_user_id, ACTOR)
  assert.equal(state.receipts.get(SUBMISSION).resolved_at, NOW)
  assert.equal(state.interactions.size, 0)
  assert.equal(state.tasks.size, 0)
})

// ---------------------------------------------------------------------------
// 6. idempotent replay — resolving an already-resolved receipt is a no-op
//    conflict; no second interaction/task/identity write.
// ---------------------------------------------------------------------------
test('replay: resolving an already-resolved receipt is a no-op conflict', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  await resolveIntake(
    {
      submissionId: SUBMISSION,
      action: { kind: 'attach', personId: PERSON_A },
      actorAppUserId: ACTOR,
    },
    runner.run,
  )
  await assert.rejects(
    resolveIntake(
      { submissionId: SUBMISSION, action: { kind: 'attach', personId: PERSON_A } },
      runner.run,
    ),
    (error) => isConflict(error) && /already resolved/.test(error.message),
  )
  // Replaying with a different action is also a no-op conflict.
  await assert.rejects(
    resolveIntake({ submissionId: SUBMISSION, action: { kind: 'reject' } }, runner.run),
    isConflict,
  )
  // No write happened on replay: the receipt and every downstream row are
  // exactly as the first resolution left them.
  assert.equal(state.receipts.get(SUBMISSION).status, 'completed')
  assert.equal(state.receipts.get(SUBMISSION).resolved_by_user_id, ACTOR)
  assert.equal(state.interactions.size, 1)
  assert.equal(state.tasks.size, 1)
  assert.equal(state.identities.size, 1)
})

// ---------------------------------------------------------------------------
// 7. duplicate submissions dedupe via interaction source identity — a receipt
//    whose canonical interaction already exists completes with the existing
//    interaction id, never a second interaction.
// ---------------------------------------------------------------------------
test('dedupe: an intake whose canonical interaction already exists reuses it', async () => {
  const state = baseState({
    receipts: new Map([[SUBMISSION, receipt({ status: 'resolution_required' })]]),
    interactions: new Map([
      [
        `website:${SUBMISSION}`,
        {
          id: EXISTING_INTERACTION,
          person_id: PERSON_A,
          property_id: PROPERTY,
          source_system: 'website',
          source_external_id: SUBMISSION,
        },
      ],
    ]),
  })
  const runner = fakeRunner(state)
  const result = await resolveIntake(
    { submissionId: SUBMISSION, action: { kind: 'attach', personId: PERSON_A } },
    runner.run,
  )
  assert.equal(result.status, 'completed')
  assert.equal(result.interactionId, EXISTING_INTERACTION)
  assert.equal(state.interactions.size, 1)
  assert.equal(state.receipts.get(SUBMISSION).interaction_id, EXISTING_INTERACTION)
  assert.equal(state.receipts.get(SUBMISSION).status, 'completed')
  // The canonical interaction already exists, so no duplicate task is created.
  assert.equal(state.tasks.size, 0)
})

// ---------------------------------------------------------------------------
// 8. create race backstop — a concurrent claim wins the email between the
//    ownership check and the insert: unique violation surfaces as a clear
//    conflict and the whole transaction rolls back (no orphan person).
// ---------------------------------------------------------------------------
test('create: a concurrent claim on the email surfaces as conflict with rollback', async () => {
  const state = baseState()
  const runner = fakeRunner(state, { failIdentityInsertFor: `email:${EMAIL}` })
  await assert.rejects(
    resolveIntake({ submissionId: SUBMISSION, action: { kind: 'create' } }, runner.run),
    (error) => isConflict(error) && /already belongs/.test(error.message),
  )
  assert.equal(state.persons.size, 2)
  assert.equal(state.receipts.get(SUBMISSION).status, 'received')
  assert.equal(state.interactions.size, 0)
  assert.equal(state.tasks.size, 0)
})

// ---------------------------------------------------------------------------
// 9. general enquiry (property-less) — create completes with no
//    property_interest side effect.
// ---------------------------------------------------------------------------
test('general enquiry: property-less intake completes without property_interest', async () => {
  const state = baseState({
    receipts: new Map([
      [SUBMISSION, receipt({ request_type: 'general_enquiry', property_id: null })],
    ]),
  })
  const runner = fakeRunner(state)
  const result = await resolveIntake(
    { submissionId: SUBMISSION, action: { kind: 'create' } },
    runner.run,
  )
  assert.equal(result.status, 'completed')
  assert.equal(state.interactions.size, 1)
  assert.equal(state.interests.size, 0)
})

// ---------------------------------------------------------------------------
// 10. actor absent (service invoked outside the session path) — resolved_at is
//     durable, resolved_by_user_id stays null.
// ---------------------------------------------------------------------------
test('actor: resolved_by_user_id stays null when no session actor is threaded', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  await resolveIntake({ submissionId: SUBMISSION, action: { kind: 'reject' } }, runner.run)
  assert.equal(state.receipts.get(SUBMISSION).status, 'rejected')
  assert.equal(state.receipts.get(SUBMISSION).resolved_by_user_id, null)
  assert.equal(state.receipts.get(SUBMISSION).resolved_at, NOW)
})

// ---------------------------------------------------------------------------
// 11. validation — malformed inputs are rejected before any write.
// ---------------------------------------------------------------------------
test('validation: malformed inputs are rejected before any write', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  await assert.rejects(
    resolveIntake({ submissionId: 'not-a-uuid', action: { kind: 'reject' } }, runner.run),
    /invalid/i,
  )
  await assert.rejects(
    resolveIntake(
      { submissionId: SUBMISSION, action: { kind: 'attach', personId: 'nope' } },
      runner.run,
    ),
    /invalid/i,
  )
  await assert.rejects(
    resolveIntake(
      { submissionId: SUBMISSION, action: { kind: 'create' }, actorAppUserId: 'nope' },
      runner.run,
    ),
    /invalid/i,
  )
  await assert.rejects(
    resolveIntake({ submissionId: SUBMISSION, action: { kind: 'unknown' } }, runner.run),
    /invalid/i,
  )
  assert.equal(state.receipts.get(SUBMISSION).status, 'received')
})

// ---------------------------------------------------------------------------
// 12. missing target person — attach to a nonexistent person is not-found.
// ---------------------------------------------------------------------------
test('attach: a nonexistent target person is a not-found conflict', async () => {
  const state = baseState()
  const runner = fakeRunner(state)
  await assert.rejects(
    resolveIntake(
      {
        submissionId: SUBMISSION,
        action: {
          kind: 'attach',
          personId: '10000000-0000-4000-8000-000000000099',
        },
      },
      runner.run,
    ),
    (error) => error instanceof PortalWriteError && error.code === 'not-found',
  )
  assert.equal(state.receipts.get(SUBMISSION).status, 'received')
})

// ---------------------------------------------------------------------------
// 13. resolution_required receipts are actionable too (flagged for a decision).
// ---------------------------------------------------------------------------
test('actionable: a resolution_required receipt resolves like a received one', async () => {
  const state = baseState({
    receipts: new Map([[SUBMISSION, receipt({ status: 'resolution_required' })]]),
  })
  const runner = fakeRunner(state)
  const result = await resolveIntake(
    { submissionId: SUBMISSION, action: { kind: 'reject' }, actorAppUserId: ACTOR },
    runner.run,
  )
  assert.deepEqual(result, { submissionId: SUBMISSION, status: 'rejected' })
  assert.equal(state.receipts.get(SUBMISSION).resolved_by_user_id, ACTOR)
})

// ---------------------------------------------------------------------------
// 14. Server-action actor wiring (AUTH-05) — the Portal entry point derives
//     resolved_by_user_id from the authenticated session, never from a
//     client-supplied actor id (audit attribution must not be forgeable).
//     Static contract check, mirroring the auth-03 action-gate parser style.
// ---------------------------------------------------------------------------
test('action: session actor is threaded and client actor id is rejected', async () => {
  const src = readFileSync(
    new URL('../../app/portal/actions.ts', import.meta.url),
    'utf8',
  )
  const actionStart = src.indexOf('export async function resolveIntakeAction')
  assert.ok(actionStart !== -1, 'resolveIntakeAction exists')
  const actionBlock = src.slice(
    actionStart,
    src.indexOf('\nexport async function', actionStart + 1),
  )
  // Threads the session actor into resolveIntake (AUTH-05 resolved_by_user_id).
  assert.match(actionBlock, /actorAppUserId: actor\.appUserId/)
  // No client-supplied actor id in the input contract.
  assert.doesNotMatch(
    actionBlock,
    /actorAppUserId\?: string \| null/,
    'client-supplied actor id must not be accepted',
  )
  // Still gated on the coarse crm.write authority (auth-command-map contract).
  assert.match(actionBlock, /portalWrite\('crm\.write'/)
})

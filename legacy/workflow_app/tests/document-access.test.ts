// ---------------------------------------------------------------------------
// SEC-MEDIA-DOC-01 — an executed contract is not served to whoever holds its id.
//
// ONE pure decision gates document downloads: session facts in, allow | deny
// with a reason out. The route owns the facts (the row exists and is a
// document; the request carries a portal session); this test interrogates the
// rule directly and the route's wiring, so the rule cannot disagree with
// itself in two places.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { decideDocumentAccess } from '@/lib/auth/document-access'
import type { DocumentAccessFacts } from '@/lib/auth/document-access'

const DECISION_MODULE = 'lib/auth/document-access.ts'
const ROUTE = 'app/api/media/documents/[id]/route.ts'

async function read(rel: string): Promise<string> {
  return readFile(new URL(`../../../${rel}`, import.meta.url), 'utf8')
}

test('SEC-MEDIA-DOC-01: a non-document media id denies not_found', () => {
  assert.deepEqual(
    decideDocumentAccess({ isDocument: false, hasPortalSession: false }),
    { allow: false, reason: 'not_found' },
  )
  assert.deepEqual(
    decideDocumentAccess({ isDocument: false, hasPortalSession: true }),
    { allow: false, reason: 'not_found' },
  )
})

test('SEC-MEDIA-DOC-01: a document with no portal session denies unauthenticated', () => {
  assert.deepEqual(
    decideDocumentAccess({ isDocument: true, hasPortalSession: false }),
    { allow: false, reason: 'unauthenticated' },
  )
})

test('SEC-MEDIA-DOC-01: a document with a portal session allows', () => {
  assert.deepEqual(decideDocumentAccess({ isDocument: true, hasPortalSession: true }), {
    allow: true,
  })
})

test('SEC-MEDIA-DOC-01: the decision is deterministic and does not mutate its facts', () => {
  const facts: DocumentAccessFacts = Object.freeze({
    isDocument: true,
    hasPortalSession: false,
  })
  const first = decideDocumentAccess(facts)
  const second = decideDocumentAccess(facts)
  assert.deepEqual(first, second)
  assert.deepEqual(facts, { isDocument: true, hasPortalSession: false })
})

test('SEC-MEDIA-DOC-01: the decision module is pure — no I/O, env, DB or framework imports', async () => {
  const source = await read(DECISION_MODULE)
  assert.ok(!/^\s*import\b/m.test(source), 'the decision module has no imports')
  assert.ok(!/\brequire\s*\(/.test(source), 'the decision module has no require()')
  assert.ok(!/\bprocess\.env\b/.test(source), 'the decision module reads no env')
  assert.ok(!/\bawait\b/.test(source), 'the decision module does no async work')
  assert.ok(!/\b(sql|fetch|next-auth)\b/.test(source), 'the decision module touches no DB or framework')
})

test('SEC-MEDIA-DOC-01: the route serves bytes only through the named decision', async () => {
  const source = await read(ROUTE)
  assert.ok(
    /import\s*\{[^}]*decideDocumentAccess[^}]*\}\s*from\s*'@\/lib\/auth\/document-access'/.test(
      source,
    ),
    'the route imports decideDocumentAccess from the decision module',
  )
  assert.ok(
    source.includes('decideDocumentAccess({ isDocument, hasPortalSession })'),
    'the route calls the decision with the session facts',
  )
  assert.ok(
    source.includes("decision.reason === 'unauthenticated'"),
    'the route branches on the decision reason',
  )
  assert.ok(source.includes('status: 401'), 'no session maps to 401')
  assert.ok(source.includes('status: 404'), 'not_found maps to 404')
})

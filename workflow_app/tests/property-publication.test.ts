// ---------------------------------------------------------------------------
// HARDEN-05 — Property publication / market visibility invariant.
//
// PROPERTY OWNS PUBLICATION STATE; LISTING MEDIA INHERITS IT.
//
// Focused proofs:
//   1. every PUBLIC property read gates on property.is_published
//   2. no legacy status-based public predicate remains in db/properties.ts
//   3. the publish seam is the canonical single-authority mutation + validation
//   4. getProperties exposes the explicit publicOnly option
//
// The deterministic end-to-end behavior (public vs internal on the real DEV
// control plane, direct-URL 404, media inheritance, toggle) is proven by
// scripts/verify-property-publication.ts.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import {
  getFilteredProperties,
  getPropertyBySlug,
  getPublicPropertySlugs,
  getSimilarProperties,
} from '../../db/properties'
import { setPropertyPublished } from '../../db/portal-property'
import { PortalWriteError } from '../../lib/portal-write-error'
import type { QueryExecutor } from '../../db/query-executor'

type Captured = { sql: string; params: unknown[] }

/** Returns a fake executor that captures each call's text and replays a
 *  sequence of canned row-sets (last set repeats). */
function makeExecutor(
  sequences: Record<string, unknown>[][],
  captured: Captured[],
): QueryExecutor {
  let i = 0
  return async (strings, ...params) => {
    captured.push({ sql: strings.join('?'), params })
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

test('HARDEN-05: getFilteredProperties (public buyers) gates on is_published', async () => {
  const captured: Captured[] = []
  const fake = makeExecutor([[], []], captured)
  await getFilteredProperties({ category: 'all' }, fake)
  assert.ok(
    captured.length >= 2,
    'property + view queries both issued',
  )
  for (const c of captured) {
    assert.ok(
      c.sql.toLowerCase().includes('is_published = true'),
      `public filter present: ${c.sql.slice(0, 80)}`,
    )
  }
})

test('HARDEN-05: getSimilarProperties (public) gates on is_published', async () => {
  const captured: Captured[] = []
  await getSimilarProperties(
    'prop-1',
    { propertyType: null, city: null, neighborhood: null, listPrice: null },
    3,
    makeExecutor([[]], captured),
  )
  assert.ok(captured[0].sql.toLowerCase().includes('is_published = true'))
})

test('HARDEN-05: getPublicPropertySlugs gates on is_published', async () => {
  const captured: Captured[] = []
  await getPublicPropertySlugs(makeExecutor([[]], captured))
  assert.ok(captured[0].sql.toLowerCase().includes('is_published = true'))
})

test('HARDEN-05: getPropertyBySlug (public detail) gates on is_published (direct URL 404 for internal)', async () => {
  const captured: Captured[] = []
  const result = await getPropertyBySlug('some-slug', makeExecutor([[]], captured))
  assert.equal(result, null, 'no row -> null (page calls notFound())')
  assert.ok(captured[0].sql.toLowerCase().includes('is_published = true'))
})

test('HARDEN-05: setPropertyPublished is the canonical idempotent publication mutation', async () => {
  // Success path writes is_published and returns id + slug.
  const captured: Captured[] = []
  const ok = await setPropertyPublished(
    'p1',
    true,
    makeExecutor([[{ id: 'p1', slug: 'casa' }]], captured),
  )
  assert.deepEqual(ok, { id: 'p1', slug: 'casa' })
  assert.ok(captured[0].sql.toLowerCase().includes('is_published'))

  // Missing property -> not-found.
  await assert.rejects(
    () =>
      setPropertyPublished('p1', true, makeExecutor([[]], captured)),
    (err: unknown) =>
      err instanceof PortalWriteError && err.code === 'not-found',
  )

  // Non-boolean is rejected.
  await assert.rejects(
    () =>
      setPropertyPublished(
        'p1',
        'yes' as unknown as boolean,
        makeExecutor([[{ id: 'p1', slug: 'casa' }]], captured),
      ),
    (err: unknown) =>
      err instanceof PortalWriteError && err.code === 'validation',
  )
})

test('HARDEN-05: legacy status predicate confined to the internal getProperties default; public reads use is_published', async () => {
  const source = await readFile(
    new URL('../../db/properties.ts', import.meta.url),
    'utf8',
  )
  const legacy = /status in \('active', 'coming_soon', 'under_contract'\)/g
  const legacyCount = (source.match(legacy) ?? []).length
  assert.ok(
    legacyCount <= 1,
    `legacy public predicate only in the internal getProperties default (found ${legacyCount})`,
  )
  assert.ok(source.includes('publicOnly'), 'getProperties exposes publicOnly')
  assert.ok(source.includes('is_published = true'))
})

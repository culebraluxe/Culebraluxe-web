import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  ENGINE_CLAIM_STALE_MS,
  listEngineRunCards,
  parsePostgresInstant,
} from '../../db/forge-engine-task-execution'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { assertDriverValueShape } from './helpers/interrupted-sequence'

// The fixture is the driver's own `timestamptz::text`, captured once by
// scripts/capture-driver-value-formats.ts. It replaces the hand-written
// timestamp literal whose `Z`-append hid the ENG-FORGE-CLAIM-CLOCK-01 defect:
// the guard fails any value in a shape the driver never emits.
const DRIVER_CAPTURE = JSON.parse(
  readFileSync(new URL('./fixtures/driver-value-formats.json', import.meta.url), 'utf8'),
) as { values: Record<string, string> }

// ---------------------------------------------------------------------------
// The engine's stale-claim clock. A Postgres timestamp text carries an offset
// already (`2026-09-17 07:16:52.653+00`); the read must parse the instant it
// names in every form the driver emits, so a claim touched seconds ago can
// never be reported stale because a suffix made Date.parse return NaN.
// ---------------------------------------------------------------------------

// One fixed instant, named in UTC. Every driver form below names this same
// instant and must parse to this same epoch.
const FIXED_ISO = '2026-09-17T07:16:52.653Z'
const FIXED = Date.parse(FIXED_ISO)
const NO_OFFSET = FIXED_ISO.replace('T', ' ').replace('Z', '')

test('parses a bare-offset timestamp to its instant', () => {
  assert.equal(parsePostgresInstant(`${NO_OFFSET}+00`), FIXED)
})

test('parses a colon-offset timestamp to its instant', () => {
  assert.equal(parsePostgresInstant(`${NO_OFFSET}+00:00`), FIXED)
})

test('parses a Z timestamp to its instant', () => {
  assert.equal(parsePostgresInstant(`${NO_OFFSET}Z`), FIXED)
})

test('parses a no-offset timestamp to its instant', () => {
  assert.equal(parsePostgresInstant(NO_OFFSET), FIXED)
})

// The fixture is the driver's real text, not a hand-normalised literal. A
// timestamp in a shape the driver never emits fails here, so the `Z`-append
// defect class cannot be reintroduced by editing the fixture.
test('the captured driver timestamp is the form the reader must parse', () => {
  const captured = DRIVER_CAPTURE.values.timestamptzText
  assertDriverValueShape(captured, 'timestamptz')
  assert.ok(Number.isFinite(parsePostgresInstant(captured)))
})

// A claim touched seconds ago, written in each driver form. The board must read
// every one of them as live: stale=false.
const secondsAgo = new Date(Date.now() - 5_000).toISOString()
const recentBase = secondsAgo.replace('T', ' ').replace('Z', '')
const recentForms: Array<[string, string]> = [
  ['bare offset', `${recentBase}+00`],
  ['colon offset', `${recentBase}+00:00`],
  ['Z', `${recentBase}Z`],
  ['no offset', recentBase],
]

function cardFor(updatedAt: string, status = 'claimed'): QueryExecutor {
  const row: QueryRow = {
    story_id: 'ENG-CLOCK',
    title: 'claim clock',
    instance_id: '11111111-1111-1111-1111-111111111111',
    node_id: 'smith',
    status,
    at: updatedAt,
    updated_at: updatedAt,
    attempts: 1,
  }
  return async () => [row]
}

test('a claim touched seconds ago is not stale in any driver format', async () => {
  for (const [label, form] of recentForms) {
    const cards = await listEngineRunCards(30, cardFor(form))
    assert.equal(cards.length, 1, `${label}: one card`)
    assert.equal(cards[0].stale, false, `${label}: not stale`)
    assert.equal(cards[0].status, 'claimed')
  }
})

test('an unparseable timestamp is not stale', async () => {
  const cards = await listEngineRunCards(30, cardFor('not-a-timestamp'))
  assert.equal(cards.length, 1)
  assert.equal(cards[0].stale, false)
})

// The window itself is unchanged: a claim genuinely older than the cleaner's
// window still reads stale, so the fix did not disable abandonment.
test('a claim older than the stale window is stale', async () => {
  const oldIso = new Date(Date.now() - ENGINE_CLAIM_STALE_MS - 60_000)
    .toISOString()
    .replace('T', ' ')
    .replace('Z', '')
  const cards = await listEngineRunCards(30, cardFor(`${oldIso}+00`))
  assert.equal(cards.length, 1)
  assert.equal(cards[0].stale, true)
})

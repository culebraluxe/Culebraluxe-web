import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { pickLane, storyFieldsFromBoardAndGit } from './orchestrate'

test('no brief → scout', () => {
  assert.equal(pickLane({ story: {} }), 'scout')
})

test('after scout with no brief still → scout', () => {
  assert.equal(pickLane({ story: {}, lastFinishedRole: 'scout' }), 'scout')
})

test('Neon brief only → smith', () => {
  const merged = storyFieldsFromBoardAndGit(
    { architectBrief: 'brief from Neon' },
    'NO-PACKET',
    '/definitely/missing',
  )
  assert.equal(pickLane({ story: merged }), 'smith')
})

test('git packet brief only → smith', () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-v3-02-'))
  try {
    const packetDir = join(root, 'docs', 'agent', 'packets')
    mkdirSync(packetDir, { recursive: true })
    writeFileSync(
      join(packetDir, 'PACKET-ONLY.md'),
      '## Architect brief\nbrief from packet\n',
    )
    const merged = storyFieldsFromBoardAndGit({}, 'PACKET-ONLY', root)
    assert.equal(merged.architectBrief, 'brief from packet')
    assert.equal(pickLane({ story: merged }), 'smith')
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('after builder → assay', () => {
  assert.equal(
    pickLane({ story: { architectBrief: 'x' }, lastFinishedRole: 'builder' }),
    'assay',
  )
})

test('after scout with brief → smith', () => {
  assert.equal(
    pickLane({
      story: { architectBrief: 'x' },
      lastFinishedRole: 'scout',
    }),
    'smith',
  )
})

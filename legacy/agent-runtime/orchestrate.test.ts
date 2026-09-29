import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { pickLane, admitExecutableContract, sha256Text, storyFieldsFromBoardAndGit } from './orchestrate'

test('no brief → scout', () => {
  assert.equal(pickLane({ story: {} }), 'scout')
})

test('after scout with no brief still → scout', () => {
  assert.equal(pickLane({ story: {}, lastFinishedRole: 'scout' }), 'scout')
})

test('Neon brief only → lead (V6 Lead PRE gate)', () => {
  const merged = storyFieldsFromBoardAndGit(
    { architectBrief: 'brief from Neon' },
    'NO-PACKET',
    '/definitely/missing',
  )
  assert.equal(pickLane({ story: merged }), 'lead')
})

test('git packet brief only → lead (V6 Lead PRE gate)', () => {
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
    assert.equal(pickLane({ story: merged }), 'lead')
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('after builder → lead (V6 Lead POST gate)', () => {
  assert.equal(
    pickLane({ story: { architectBrief: 'x' }, lastFinishedRole: 'builder' }),
    'lead',
  )
})

test('after scout with brief → lead (V6 Lead PRE gate)', () => {
  assert.equal(
    pickLane({
      story: { architectBrief: 'x' },
      lastFinishedRole: 'scout',
    }),
    'lead',
  )
})

test('admission seam stamps packetSha from Git bytes and Neon durable truth wins', () => {
  const markdown = '## Architect brief\nbrief from packet\n\n## Assay commands\n- pnpm exec tsx --test x\n'
  const expectedSha = sha256Text(markdown)
  const { contract, missing } = admitExecutableContract({}, markdown)
  assert.equal(contract.packetSha, expectedSha)
  assert.equal(contract.architectBrief, 'brief from packet')
  assert.ok(missing.includes('missing-test-mode') || missing.includes('missing-acceptance-criteria'))

  const neonSha = 'neon-persisted-sha'
  const neonWins = admitExecutableContract(
    { architectBrief: 'neon brief', packetSha: neonSha },
    markdown,
  )
  assert.equal(neonWins.contract.packetSha, neonSha)
  assert.equal(neonWins.contract.architectBrief, 'neon brief')
})

test('admission seam reports missing executable facts without fabricating them', () => {
  const { contract, missing } = admitExecutableContract({}, null)
  assert.equal(contract.packetSha ?? null, null)
  assert.ok(missing.includes('missing-architect-brief'))
  assert.ok(missing.includes('missing-assay-plan'))
})

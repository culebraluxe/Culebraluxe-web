import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import test from 'node:test'

import {
  BLOCK_END,
  BLOCK_START,
  GUARDRAILS,
  hasVendorBlock,
  orphanedGuardrails,
  renderVendorBlock,
  upsertVendorBlock,
  vendorBlockDrifted,
} from '../lib/agent-vendor-block'
import { planSync } from './forge-sync-agents'

// ---------------------------------------------------------------------------
// The vendor-block mechanism, tested for the two properties that make it safe:
// an idempotent re-run must not touch the file, and anything the block asserts must
// still be true in AGENTS.md.
// ---------------------------------------------------------------------------

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
const agentsMd = readFileSync(join(repoRoot, 'AGENTS.md'), 'utf8')

test('the block points at the handbook instead of copying it', () => {
  const block = renderVendorBlock()
  for (const path of ['AGENTS.md', 'docs/agent/ORIENTATION.md', 'docs/agent/MEMORY.md']) {
    assert.ok(block.includes(path), `the block should name ${path}`)
  }
  assert.ok(block.includes(GUARDRAILS[0].text))
  // A pointer, not a second handbook: the block stays short on purpose.
  assert.ok(block.split('\n').length < 40, 'a pointer block that grows into the handbook is the failure mode')
})

test('upsert appends when there is no block, and keeps the hand-written text', () => {
  const original = '# Adapter\n\nRead @AGENTS.md.\n'
  const next = upsertVendorBlock(original, renderVendorBlock())
  assert.ok(next.startsWith('# Adapter'))
  assert.ok(next.includes('Read @AGENTS.md.'))
  assert.ok(next.includes(BLOCK_START) && next.includes(BLOCK_END))
})

test('upsert is a no-op on an already-current file', () => {
  const written = upsertVendorBlock('# Adapter\n\nRead @AGENTS.md.\n', renderVendorBlock())
  assert.equal(upsertVendorBlock(written, renderVendorBlock()), written)
})

test('upsert replaces only the managed span', () => {
  const written = upsertVendorBlock('# Adapter\n\nbefore\n', renderVendorBlock())
  const edited = written.replace(BLOCK_END, `hand edit\n${BLOCK_END}`)
  const repaired = upsertVendorBlock(edited, renderVendorBlock())
  assert.ok(repaired.includes('before'))
  assert.ok(!repaired.includes('hand edit'), 'the edited span should be replaced, not preserved')
  assert.notEqual(repaired, edited)
})

test('a malformed block append rather than clobbers the file', () => {
  const orphanStart = `# Adapter\n\n${BLOCK_START}\nno end marker here\n`
  const next = upsertVendorBlock(orphanStart, renderVendorBlock())
  assert.ok(next.includes('no end marker here'), 'an unclosed marker must not eat the file')
  assert.ok(next.includes(BLOCK_END))
})

test('a hand edit is drift; a fresh render is not', () => {
  const written = upsertVendorBlock('# Adapter\n', renderVendorBlock())
  assert.equal(vendorBlockDrifted(written), false)
  const edited = written.replace('Only the Builder role commits', 'Anyone may commit')
  assert.equal(vendorBlockDrifted(edited), true)
  assert.equal(hasVendorBlock(edited), true)
})

test('the real AGENTS.md anchors every guardrail', () => {
  assert.deepEqual(orphanedGuardrails(agentsMd), [])
})

test('removing the handbook sentence orphans its guardrail', () => {
  const sabotaged = agentsMd.replace('Commit secrets or `.env.local`', 'Commit anything you like')
  const orphaned = orphanedGuardrails(sabotaged)
  assert.equal(orphaned.length, 1)
  assert.equal(orphaned[0].anchoredBy, 'Commit secrets or `.env.local`')
})

test('planSync: no block means write, identical means nothing, edited means drift', () => {
  const withBlock = upsertVendorBlock('# Adapter\n', renderVendorBlock())
  const edited = withBlock.replace('Only the Builder role commits', 'Anyone may commit')
  const plan = planSync({
    present: new Map([
      ['CLAUDE.md', edited],
      ['WARP.md', '# Adapter\n'],
    ]),
    absent: ['GEMINI.md'],
    agentsMd,
  })
  assert.deepEqual(
    plan.files.map((file) => [file.path, file.status]),
    [
      ['CLAUDE.md', 'drifted'],
      ['WARP.md', 'would-write'],
    ],
  )
  assert.equal(plan.writes.length, 2)
  assert.deepEqual(plan.orphaned, [])
  assert.deepEqual(plan.absent, ['GEMINI.md'])
})

test('planSync leaves a current file alone', () => {
  const current = upsertVendorBlock('# Adapter\n', renderVendorBlock())
  const plan = planSync({ present: new Map([['CLAUDE.md', current]]), absent: [], agentsMd })
  assert.deepEqual(plan.files, [{ path: 'CLAUDE.md', status: 'unchanged', hasBlock: true }])
  assert.equal(plan.writes.length, 0)
})

import assert from 'node:assert/strict'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

import {
  markerRefusal,
  PROTECTED_DIRECTORIES,
  PROTECTED_FILES,
  protectionRefusals,
} from './protected-files'

// ---------------------------------------------------------------------------
// THE DO-NOT-BREAK GATE.
//
// The captain asked for a list of files not to touch. A list that is only read does not survive a
// mechanical mistake, so these tests ARE the enforcement: they run inside `pnpm test:harness`, which
// the release build runs before anything ships. If a protected file loses its first line, is renamed
// away, or something writes a non-manifest into the manifest directory, the build stops and says
// which file and why it mattered — instead of a human noticing weeks later that a table got shorter.
// ---------------------------------------------------------------------------

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

test('every protected file in the real repo is intact', () => {
  assert.deepEqual(
    protectionRefusals(root),
    [],
    'a protected file lost its marker: restore it (git has it) rather than editing this list',
  )
})

test('every protection states its marker AND its reason, so the list stays a conversation', () => {
  for (const file of PROTECTED_FILES) {
    assert.ok(file.marker.length > 0, `${file.path} must name the marker it protects`)
    assert.ok(
      file.why.trim().length > 20,
      `${file.path} must say WHY (a one-word reason is a list nobody can weigh later)`,
    )
  }
  for (const dir of PROTECTED_DIRECTORIES) {
    assert.ok(dir.marker.length > 0, `${dir.dir} must name the marker every file in it must carry`)
    assert.ok(dir.why.trim().length > 20, `${dir.dir} must say why the rule exists`)
  }
})

test('markerRefusal: a missing protected file is a break, not a cleanup', () => {
  const refusal = markerRefusal('docs/agent/releases.md', null, '# Releases')
  assert.ok(refusal, 'absence must be refused')
  assert.match(String(refusal), /MISSING/)
})

test('markerRefusal: a file written by something that did not know what it was is refused, and shows what it got', () => {
  const refusal = markerRefusal(
    'docs/agent/COLUMN-WRITER-AUDIT.md',
    '# Scope manifest — COLUMN-WRITER-AUDIT\n',
    '# Column writer audit',
  )
  assert.ok(refusal, 'the wrong generator writing over the audit is exactly the case')
  assert.match(String(refusal), /Scope manifest — COLUMN-WRITER-AUDIT/, 'it quotes the first line it found')
})

test('markerRefusal: an intact file passes, including one with more content after the marker', () => {
  assert.equal(markerRefusal('x.md', '# Column writer audit\n\n| table |\n', '# Column writer audit'), null)
  assert.equal(markerRefusal('x.md', 'stray no newline', 'stray'), null)
})

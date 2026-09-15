import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import test from 'node:test'

import {
  HANDBOOK_PATHS,
  citedPaths,
  isIdentifierToken,
  lineCitations,
  manifestFileName,
  rankManifestEntries,
  renderManifest,
  tokenize,
} from '../lib/scope-manifest'
import { buildManifest, manifestDrift, parseArgs } from './forge-manifest'

// ---------------------------------------------------------------------------
// The manifest's own tests. Two kinds, both needed:
//   - the pure ranking/rendering rules, with fixtures that MUST fail;
//   - the real repo, because a ranking that only works on fixtures is a ranking
//     nobody has seen answer a real question.
// ---------------------------------------------------------------------------

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()

test('the always-read handbook comes first and is never dropped', () => {
  const rows = rankManifestEntries({
    scope: 'X-01',
    packetPath: 'docs/agent/packets/X-01.md',
    commitPaths: ['lib/thing.ts'],
    exists: () => true,
  })
  assert.deepEqual(
    rows.slice(0, HANDBOOK_PATHS.length).map((row) => row.path),
    [...HANDBOOK_PATHS],
  )
  assert.equal(rows[HANDBOOK_PATHS.length].lane, 'packet')
})

test('a row for a path that is not on disk is marked missing', () => {
  const rows = rankManifestEntries({
    scope: 'X-01',
    packetPath: 'docs/agent/packets/X-01.md',
    commitPaths: ['lib/gone.ts'],
    exists: (path) => path !== 'lib/gone.ts',
  })
  const gone = rows.find((row) => row.path === 'lib/gone.ts')
  assert.ok(gone, 'expected the commit row to exist')
  assert.equal(gone.missing, true)
  assert.equal(rows.find((row) => row.path === 'lib/thing.ts'), undefined)
})

test('a cited path outranks a lexical match that scores far higher', () => {
  const corpus = [
    { path: 'docs/agent/loud.md', content: 'widget widget widget widget widget widget widget' },
    { path: 'docs/agent/quiet.md', content: 'widget' },
  ]
  const rows = rankManifestEntries({
    scope: 'X-01',
    packetContent: 'The widget is changed in `lib/quiet-target.ts`.',
    corpus,
    exists: () => true,
  })
  const citedIndex = rows.findIndex((row) => row.path === 'lib/quiet-target.ts')
  const lexicalIndex = rows.findIndex((row) => row.lane === 'lexical')
  assert.ok(citedIndex >= 0, 'the cited path should be a row')
  assert.ok(lexicalIndex > citedIndex, 'structural rows must precede lexical rows')
})

test('an identifier token outranks a prose token at the same frequency', () => {
  const corpus = [
    { path: 'docs/agent/a.md', content: 'storyboard_story is the table' },
    { path: 'docs/agent/b.md', content: 'storyboard is the table' },
  ]
  const rows = rankManifestEntries({
    scope: 'X-01',
    packetContent: 'storyboard_story storyboard',
    corpus,
    exists: () => true,
    lexicalLimit: 5,
  })
  const lexical = rows.filter((row) => row.lane === 'lexical')
  assert.equal(lexical[0].path, 'docs/agent/a.md')
  assert.ok(isIdentifierToken('storyboard_story'))
  assert.ok(!isIdentifierToken('storyboard'))
})

test('citedPaths keeps repo paths and drops routes, globs and placeholders', () => {
  const paths = citedPaths(
    'see `lib/scope-manifest.ts`, `docs/agent/packets/<ID>.md`, `/buyers`, `docs/agent/manifest/*.md`, `AGENTS.md` and `app/portal/tech/actions.ts:120`',
  )
  assert.ok(paths.includes('lib/scope-manifest.ts'))
  assert.ok(paths.includes('app/portal/tech/actions.ts'), 'a line-suffixed citation keeps the path')
  assert.ok(!paths.includes('AGENTS.md'), 'a bare filename is not a repo-relative path')
  assert.ok(!paths.some((path) => path.includes('<') || path.includes('*')))
  assert.ok(!paths.some((path) => path.startsWith('/')))
})

test('line citations parse a range and a single line', () => {
  const citations = lineCitations('`lib/scope-manifest.ts:40-52` and `scripts/forge-manifest.ts:9`')
  assert.equal(citations.length, 2)
  assert.deepEqual(
    citations.map((citation) => [citation.path, citation.start, citation.end]),
    [
      ['lib/scope-manifest.ts', 40, 52],
      ['scripts/forge-manifest.ts', 9, 9],
    ],
  )
})

test('renderManifest says MISSING on the row instead of dropping it', () => {
  const markdown = renderManifest(
    [
      {
        path: 'lib/gone.ts',
        lane: 'cited',
        detail: 'cited by X-01',
        lastTouched: null,
        missing: true,
        score: 0,
      },
    ],
    {
      scope: 'X-01',
      generatedAt: '2026-09-15 00:00:00Z',
      commit: 'abc1234',
      branch: 'main',
      dirty: false,
      command: 'pnpm forge:manifest X-01',
    },
  )
  assert.match(markdown, /`lib\/gone\.ts` \*\*MISSING\*\*/)
  assert.match(markdown, /GENERATED FILE/)
})

test('manifestFileName keeps a story id and neutralises a path', () => {
  assert.equal(manifestFileName('PIRATE-01'), 'PIRATE-01')
  assert.equal(manifestFileName('docs/agent/packets'), 'docs-agent-packets')
  assert.equal(manifestFileName('   '), 'all')
})

test('parseArgs: --check-all needs no scope; the default scope is all', () => {
  assert.deepEqual(parseArgs(['--check-all']), { check: false, checkAll: true, json: false, scope: '' })
  assert.equal(parseArgs([]).scope, 'all')
  assert.equal(parseArgs(['PIRATE-01', '--check']).scope, 'PIRATE-01')
  assert.equal(parseArgs(['PIRATE-01', '--check']).check, true)
  assert.equal(parseArgs(['PIRATE-01', '--format', 'json']).json, true)
})

test('drift is reported as added and removed rows, not as a wall of text', () => {
  const before = '- `AGENTS.md` — handbook · always-read · last touched 2026-09-01\n'
  const after =
    '- `AGENTS.md` — handbook · always-read · last touched 2026-09-01\n' +
    '- `lib/new.ts` — cited · cited by X-01 · last touched 2026-09-15\n'
  const drift = manifestDrift(before, after)
  assert.deepEqual(drift.added, ['- `lib/new.ts` — cited · cited by X-01'])
  assert.deepEqual(drift.removed, [])
})

test('the real repo: PIRATE-01 ranks its packet first and cites real files', () => {
  const built = buildManifest(repoRoot, 'PIRATE-01')
  const packetIndex = built.entries.findIndex((entry) => entry.lane === 'packet')
  assert.equal(built.entries[packetIndex].path, 'docs/agent/packets/PIRATE-01.md')
  assert.ok(
    built.entries.some((entry) => entry.path === 'scripts/forge-packet-lint.ts' && entry.lane === 'cited'),
    'the packet cites the lint, and the manifest should say so',
  )
  assert.deepEqual(
    built.entries.filter((entry) => entry.missing).map((entry) => entry.path),
    [],
    'every row of the real manifest must be a file that exists',
  )
  assert.ok(built.entries.length > HANDBOOK_PATHS.length, 'a packet scope should rank more than the handbook')
})

test('the real repo: identifiers survive tokenizing', () => {
  const tokens = tokenize('The `forge_batch_item` row and V5-23 and storyboard_story')
  for (const token of ['forge_batch_item', 'v5-23', 'storyboard_story']) {
    assert.ok(tokens.includes(token), `${token} should survive tokenizing, got: ${tokens.join(', ')}`)
  }
})

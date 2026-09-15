import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import test from 'node:test'

import { renderVendorBlock, upsertVendorBlock } from '../lib/agent-vendor-block'
import { lintHarness, loadBaseline, loadHarnessFiles } from './forge-packet-lint'

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()

// ---------------------------------------------------------------------------
// The lint's own tests. Each rule gets a fixture that MUST fail and one that must pass, because a gate
// nobody has seen fail is indistinguishable from decoration - the exact criticism that produced it.
// ---------------------------------------------------------------------------

const packet = (body: string) => ({
  path: 'docs/agent/packets/TEST-01.md',
  content: `# TEST-01 — a fixture\n\n${body}\n`,
})

test('a packet naming a skill that does not exist fails', () => {
  const findings = lintHarness({
    files: [packet('## Skills\n\nneon, quantum-neural-wiki\n')],
    knownSkills: ['neon', 'forms'],
  })
  const hit = findings.find((f) => f.rule === 'skills-unknown')
  assert.ok(hit, 'expected an unknown-skill finding')
  assert.equal(hit.level, 'fail')
  assert.match(hit.message, /quantum-neural-wiki/)
})

test('a packet listing more than three skills fails', () => {
  const findings = lintHarness({
    files: [packet('## Skills\n\nneon, forms, workflow, ui\n')],
    knownSkills: ['neon', 'forms', 'workflow', 'ui', 'planner'],
  })
  const hit = findings.find((f) => f.rule === 'skills-too-many')
  assert.ok(hit, 'expected a too-many-skills finding')
  assert.match(hit.message, /lists 4 skills; the cap is 3/)
})

test('a packet with three real skills passes', () => {
  const findings = lintHarness({
    files: [packet('## Skills\n\nneon, forms, planner\n')],
    knownSkills: ['neon', 'forms', 'planner'],
  })
  assert.deepEqual(findings.filter((f) => f.level === 'fail'), [])
})

test('a MEMORY.md entry with no date fails, a dated one passes', () => {
  const undated = lintHarness({
    files: [
      {
        path: 'docs/agent/MEMORY.md',
        content: '- **something we learned the hard way and must never forget because it cost hours**\n',
      },
    ],
  })
  assert.ok(undated.find((f) => f.rule === 'memory-entry-undated'))

  const dated = lintHarness({
    files: [
      {
        path: 'docs/agent/MEMORY.md',
        content: '- **2026-09-15 (a dated fact):** something we learned and can now find again\n',
      },
    ],
  })
  assert.deepEqual(dated.filter((f) => f.rule === 'memory-entry-undated'), [])
})

test('secret-shaped tokens fail, in any harness file', () => {
  const findings = lintHarness({
    files: [
      { path: 'AGENTS.md', content: 'token: sk-abcdefghijklmnopqrstuvwx\n' },
      { path: 'docs/agent/packets/TEST-02.md', content: 'db: postgres://user:pw@host/db\n' },
    ],
  })
  assert.equal(findings.filter((f) => f.rule === 'secret-shape').length, 2)
})

test('telling Inspector to commit fails; forbidding it does not', () => {
  const instruction = lintHarness({
    files: [packet('## Loop\n\nInspector: verify the diff, then git commit the fixes.\n')],
  })
  assert.ok(instruction.find((f) => f.rule === 'non-builder-commit-instruction'))

  const prohibition = lintHarness({
    files: [packet('## Loop\n\nNever create a git commit as Scout, Assay or Inspector.\n')],
  })
  assert.deepEqual(prohibition.filter((f) => f.rule === 'non-builder-commit-instruction'), [])
})

test('the skills directory drift is reported as a warning, never a failure', () => {
  const findings = lintHarness({
    files: [
      { path: 'docs/agent/skills/neon.md', content: '# neon\n' },
      { path: 'docs/agent/skills/semgrep.md', content: '# semgrep\n' },
    ],
    knownSkills: ['neon', 'ui'],
  })
  const warns = findings.filter((f) => f.level === 'warn').map((f) => f.rule).sort()
  // rule 11 adds one warning per pack here: neither fixture names a repo path, which is
  // exactly the "describes code it does not point at" case. Still warnings, never failures.
  assert.deepEqual(warns, [
    'known-skill-has-no-file',
    'skill-file-not-in-known',
    'skill-not-anchored',
    'skill-not-anchored',
  ])
  assert.deepEqual(findings.filter((f) => f.level === 'fail'), [])
})

test('a baselined failure is reported, not blocked', () => {
  const files = [packet('## Skills\n\nneon, quantum-neural-wiki\n')]
  const failing = lintHarness({ files, knownSkills: ['neon'] })
  assert.equal(failing.filter((f) => f.level === 'fail').length, 1)

  const baselined = lintHarness({
    files,
    knownSkills: ['neon'],
    baseline: ['docs/agent/packets/TEST-01.md::skills-unknown'],
  })
  assert.deepEqual(baselined.filter((f) => f.level === 'fail'), [])
  assert.match(baselined.find((f) => f.rule === 'skills-unknown')?.message ?? '', /pre-existing \(baselined\)/)
})

test('a map page citing a file that does not exist fails; real paths pass', () => {
  const missing = lintHarness({
    files: [
      {
        path: 'docs/agent/MAP-test.md',
        content: 'Open `services/ghost/ghost-service.ts` first.\n',
      },
    ],
  })
  const hit = missing.find((f) => f.rule === 'map-cites-missing-path')
  assert.ok(hit, 'expected the map rule to catch a path that does not exist')
  assert.match(hit.message, /services\/ghost/)

  const real = lintHarness({
    files: [
      {
        path: 'docs/agent/MAP-test.md',
        content: 'Open `services/property/property-service.ts` or `db/storyboard.ts`.\n',
      },
    ],
  })
  assert.deepEqual(real.filter((f) => f.rule === 'map-cites-missing-path'), [])
})

test('the real map pages only point at files that exist', () => {
  // The map is only worth having if it cannot point into thin air. This is the test that makes the
  // claim mechanical: it scans ORIENTATION.md and MAP-*.md exactly as the CLI does.
  const mapPages = loadHarnessFiles().filter((f) => /docs\/agent\/(ORIENTATION|MAP-)/.test(f.path))
  assert.ok(mapPages.length >= 3, `expected to have found the map pages, found ${mapPages.length}`)
  const findings = lintHarness({ files: mapPages })
  assert.deepEqual(
    findings.filter((f) => f.rule === 'map-cites-missing-path').map((f) => f.message),
    [],
  )
})

test('the real repo passes the lint with its recorded debt', () => {
  // The gate has to be usable on HEAD today, or it gets switched off on day one. Warnings are expected
  // (the skills-directory drift, and the baselined packets); failures are not.
  const files = loadHarnessFiles()
  const findings = lintHarness({ files, baseline: loadBaseline() })
  const failures = findings.filter((f) => f.level === 'fail')
  assert.deepEqual(
    failures.map((f) => `${f.file}: ${f.rule}`),
    [],
  )
  assert.ok(files.length > 10, 'expected to have scanned the harness')
})

// --- rules 8-11, added by PIRATE-01 for the generated artefacts -----------------

test('a manifest row naming a path that is not on disk fails', () => {
  const findings = lintHarness({
    files: [
      {
        path: 'docs/agent/manifest/TEST-01.md',
        content: '- `lib/scope-manifest.ts` — cited · cited by TEST-01 · last touched 2026-09-15\n' +
          '- `services/ghost/ghost.ts` — cited · cited by TEST-01 · last touched 2026-09-15\n',
      },
    ],
  })
  const hits = findings.filter((f) => f.rule === 'manifest-cites-missing-path')
  assert.equal(hits.length, 1)
  assert.match(hits[0].message, /services\/ghost\/ghost\.ts/)
})

test('a vendor block that drifted from a fresh render fails', () => {
  const block = renderVendorBlock()
  const fresh = upsertVendorBlock('# Adapter\n', block)
  const edited = fresh.replace('Only the Builder role commits', 'Anyone may commit')

  const ok = lintHarness({ files: [{ path: 'AGENTS.md', content: readFileSync(join(repoRoot, 'AGENTS.md'), 'utf8') }, { path: 'CLAUDE.md', content: fresh }] })
  assert.deepEqual(ok.filter((f) => f.rule === 'vendor-block-drift'), [])

  const drifted = lintHarness({
    files: [
      { path: 'AGENTS.md', content: readFileSync(join(repoRoot, 'AGENTS.md'), 'utf8') },
      { path: 'CLAUDE.md', content: edited },
    ],
  })
  assert.equal(drifted.filter((f) => f.rule === 'vendor-block-drift').length, 1)
})

test('a guardrail whose handbook sentence is gone fails', () => {
  const findings = lintHarness({
    files: [
      { path: 'AGENTS.md', content: readFileSync(join(repoRoot, 'AGENTS.md'), 'utf8').replace('Commit secrets or `.env.local`', 'Commit anything') },
    ],
  })
  const hit = findings.find((f) => f.rule === 'guardrail-anchor-missing')
  assert.ok(hit, 'expected the anchor rule to catch a removed handbook sentence')
  assert.match(hit.message, /Commit secrets/)
})

test('a citation past the end of a file fails; a real range passes', () => {
  const past = lintHarness({
    files: [packet('## Context refs\n\nsee `lib/scope-manifest.ts:99999`\n')],
  })
  assert.ok(past.find((f) => f.rule === 'evidence-line-past-eof'), 'expected the range to be checked')

  const real = lintHarness({
    files: [packet('## Context refs\n\nsee `lib/scope-manifest.ts:1-5`\n')],
  })
  assert.deepEqual(real.filter((f) => f.rule === 'evidence-line-past-eof'), [])
})

test('a citation to a file that exists nowhere fails, a bare-name shorthand does not', () => {
  const gone = lintHarness({
    files: [packet('## Context refs\n\nsee `engine-that-never-was.ts:1966`\n')],
  })
  assert.ok(gone.find((f) => f.rule === 'evidence-cites-missing-file'))

  // Shorthand convention: 15 of the 16 hits on the first live run were this, which is why
  // the resolver exists. `scope-manifest.ts` exists once, so its range is verifiable.
  const shorthand = lintHarness({
    files: [packet('## Context refs\n\nsee `scope-manifest.ts:1-5`\n')],
  })
  assert.deepEqual(shorthand.filter((f) => f.level === 'fail'), [])
})

test('a skill pack with no existing repo path is a warning, not a failure', () => {
  const findings = lintHarness({
    files: [{ path: 'docs/agent/skills/forms.md', content: '# forms\n\nAsk the user first.\n' }],
    knownSkills: ['forms'],
  })
  const hit = findings.find((f) => f.rule === 'skill-not-anchored')
  assert.ok(hit, 'expected the anchoring warning')
  assert.equal(hit.level, 'warn')
})

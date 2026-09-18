import assert from 'node:assert/strict'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

import { SECTIONS, sectionForFile, sectionReport, sectionsForPaths } from './test-sections'

// ---------------------------------------------------------------------------
// THE TAXONOMY GATE.
//
// 385 test files, ~3,300 tests. The captain asked for APP and FORGE to be separable, and by section so
// we stop killing time. The only way that stays true is if EVERY test file is classified: an
// unclassified file is a file no section runs, i.e. a test nobody will ever run again. These run in
// pnpm test:harness, so a new test file that matches no rule stops the line until it is placed.
// ---------------------------------------------------------------------------

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

test('every test file in the repo belongs to exactly one section', () => {
  const report = sectionReport(root)
  assert.deepEqual(
    report.unclassified,
    [],
    'a test file matched no rule: add one in scripts/test-sections.ts (or an EXPLICIT entry with its reason)',
  )
  assert.ok(report.total > 300, `expected the real test tree, saw ${report.total} files`)
})

test('the APP and FORGE halves are both real, and nothing is in neither', () => {
  const report = sectionReport(root)
  const byArea = SECTIONS.reduce<Record<string, number>>((acc, s) => {
    acc[s.area] = (acc[s.area] ?? 0) + (report.bySection[s.name]?.length ?? 0)
    return acc
  }, {})
  assert.ok((byArea.APP ?? 0) > 50, `APP should hold the app tests, saw ${byArea.APP ?? 0}`)
  assert.ok((byArea.FORGE ?? 0) > 50, `FORGE should hold the engine tests, saw ${byArea.FORGE ?? 0}`)
  assert.equal(
    (byArea.APP ?? 0) + (byArea.FORGE ?? 0) + (byArea.HARNESS ?? 0),
    report.total,
    'every file must land in one of the three areas',
  )
})

test('every section is populated, because an empty section is a lie in the list', () => {
  const report = sectionReport(root)
  for (const section of SECTIONS) {
    assert.ok(
      (report.bySection[section.name]?.length ?? 0) > 0,
      `${section.name} is empty — either it is not a section, or the rule that fills it is wrong`,
    )
    assert.ok(section.about.length > 20, `${section.name} must say what it is for`)
  }
})

test('sectionForFile: a fence path keeps its section even after the story is long done', () => {
  // The reason this is a mapping and not a move: these exact paths are inside stories' frozen proofs.
  assert.equal(sectionForFile('workflow_app/tests/claim-clock.test.ts'), 'forge-engine')
  assert.equal(sectionForFile('workflow_app/tests/qa-disposition-vocab.test.ts'), 'forge-verify')
  assert.equal(sectionForFile('agent-runtime/lead-decision.test.ts'), 'forge-runtime')
  assert.equal(sectionForFile('scripts/protected-files.test.ts'), 'harness')
  assert.equal(sectionForFile('workflow_app/tests/clients-pagination.test.ts'), 'app-crm')
  assert.equal(sectionForFile('workflow_app/tests/auth-boundary.test.ts'), 'app-identity')
})

test('sectionsForPaths: a change runs the sections it can affect, and says so by area', () => {
  assert.deepEqual(sectionsForPaths(root, ['workflow_app/forge/forge-executor.ts']), [
    'forge-engine',
    'forge-verify',
  ])
  assert.deepEqual(sectionsForPaths(root, ['agent-runtime/invoker.ts']), ['forge-runtime'])
  assert.deepEqual(sectionsForPaths(root, ['components/portal/x.tsx']), ['app-core'])
  assert.deepEqual(sectionsForPaths(root, ['workflow_app/tests/claim-clock.test.ts']), ['forge-engine'])
  assert.deepEqual(sectionsForPaths(root, ['docs/agent/MEMORY.md']), [], 'docs change no tests')
})

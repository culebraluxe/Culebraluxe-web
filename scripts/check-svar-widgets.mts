import React from 'react'
import { renderToString } from 'react-dom/server'
import { Gantt } from '@svar-ui/react-gantt'
import { Filemanager } from '@svar-ui/react-filemanager'

import { mapProjectToTimeline, TIMELINE_COLUMNS } from '../ui/projects/timeline-projection'
import { mapProjectToFileTree } from '../ui/projects/documents-projection'
import type { ProjectPlan, ProjectWorkNode } from '../ui/projects/model'

// ---------------------------------------------------------------------------
// SVAR widget acceptance check.    pnpm check:widgets
//
// Does each vendor widget actually ACCEPT what our projections feed it?
//
// Why this exists: the Timeline pane shipped once and blew up in the browser with
// "null is not an object (evaluating 'e.forEach')". The unit tests could not catch
// it — the projection was internally consistent, and the crash lived inside the
// widget's OWN store, on a field COMBINATION (`open: true` on a LEAF task, which
// the store clears to `data: null` and then recurses into without a null guard).
// Only rendering the real component against the real projection finds that class.
//
// These widgets SSR-render, so this is ~1s with no browser and no DB. ADD A CASE
// for each new widget: render it with the real projection's output, plus a control
// (the vendor's own example) so a broken harness is never mistaken for a data bug.
//
// Note: every SVAR package's CJS `main` points at `dist/index.cjs.js`, which is NOT
// shipped (the file is `dist/index.cjs`), so a plain require fails for all of them.
// Next uses `exports.import` -> the ESM build, so the app is unaffected; this file
// is .mts precisely so it loads that same ESM build.
// ---------------------------------------------------------------------------

const failures: string[] = []

function check(name: string, render: () => string): void {
  try {
    const html = render()
    console.log(`  OK      ${name} (${html.length} chars)`)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    console.log(`  THREW   ${name}: ${message}`)
    failures.push(`${name}: ${message}`)
  }
}

// Renders the SAME configuration the app ships, including its column set — a check
// that passes different props than production is testing a fiction.
const gantt = (tasks: unknown[], links: unknown[]): (() => string) => () =>
  renderToString(
    React.createElement(Gantt as never, {
      tasks,
      links,
      columns: TIMELINE_COLUMNS,
      readonly: true,
    } as never),
  )

const filemanager = (data: unknown[]): (() => string) => () =>
  renderToString(React.createElement(Filemanager as never, { data, readonly: true } as never))

const node = (
  id: string,
  title: string,
  overrides: Partial<ProjectWorkNode> = {},
): ProjectWorkNode => ({ id, title, type: 'task', status: 'not-started', ...overrides })

const project: ProjectPlan = {
  id: 'jessica-iverson-listing',
  title: 'Jessica Iverson Listing',
  kind: 'LISTING',
  status: 'active',
  progress: 43,
  phaseLabel: 'Open',
  workNodes: [
    node('parties', 'Clients / Parties', { status: 'complete', note: 'Seller identity confirmed.' }),
    node('property', 'Property'),
    node('agreement', 'Listing Agreement', {
      status: 'in-progress',
      children: [node('signature', 'Seller Signature', { status: 'waiting' })],
    }),
    node('media', 'Cabinet + Photos'),
    node('marketing', 'Coming Soon / Marketing'),
    node('accounting', 'Commission / Accounting'),
  ],
}

// --- Gantt (Timeline tab) -------------------------------------------------

console.log('GANTT — Timeline tab')
console.log(" control (the vendor's own example; if this fails the harness is wrong)")
check(
  'vendor example',
  gantt(
    [
      { id: 1, text: 'Project', type: 'summary', parent: 0, open: true },
      { id: 10, text: 'Task', type: 'task', parent: 1, start: new Date(2026, 3, 2), duration: 3, progress: 40 },
    ],
    [{ id: 1, source: 10, target: 11, type: 'e2s' }],
  ),
)

console.log(' real projection')
const undated = mapProjectToTimeline(project)
console.log(`  shape   ${undated.tasks.length} tasks, ${undated.links.length} links, synthetic=${undated.synthetic}`)
check('undated project (sample schedule)', gantt(undated.tasks, undated.links))

const dated = mapProjectToTimeline({
  ...project,
  workNodes: [
    node('parties', 'Clients / Parties', { status: 'complete', dueAt: '2026-09-10T00:00:00.000Z' }),
    node('agreement', 'Listing Agreement', {
      status: 'in-progress',
      dueAt: '2026-09-14T00:00:00.000Z',
      children: [
        node('signature', 'Seller Signature', { status: 'waiting', dueAt: '2026-09-12T00:00:00.000Z' }),
      ],
    }),
    node('media', 'Cabinet + Photos', { dueAt: '2026-09-18T00:00:00.000Z' }),
  ],
})
console.log(`  shape   ${dated.tasks.length} tasks, ${dated.links.length} links, synthetic=${dated.synthetic}`)
check('dated project', gantt(dated.tasks, dated.links))

console.log(' negative control (the defect that shipped, kept so it stays known)')
let openLeafStillThrows = true
try {
  gantt(
    [
      { id: 1, text: 'P', type: 'summary', parent: 0, open: true },
      { id: 2, text: 'Leaf', type: 'task', parent: 1, start: new Date(2026, 3, 2), duration: 3, open: true },
    ],
    [],
  )()
  openLeafStillThrows = false
} catch {
  openLeafStillThrows = true
}
console.log(
  openLeafStillThrows
    ? '  OK      open on a LEAF still throws (expected — why leaves are never open)'
    : '  NOTE    the vendor no longer throws here; the open-leaf rule may be relaxable',
)

// --- Filemanager (Documents tab) -----------------------------------------

console.log('\nFILEMANAGER — Documents tab')
console.log(' real projection')
const sampleCabinet = mapProjectToFileTree(project)
console.log(`  shape   ${sampleCabinet.files.length} entities, synthetic=${sampleCabinet.synthetic}`)
check('sample cabinet (no linked documents)', filemanager(sampleCabinet.files))

const realDocs = mapProjectToFileTree({
  ...project,
  documents: [
    { id: 'd1', title: 'Listing Agreement', state: 'issued', propertyId: 'x', createdAt: '2026-09-02T00:00:00.000Z' },
    { id: 'd2', title: 'Disclosure', state: 'draft', propertyId: 'x', createdAt: '2026-09-03T00:00:00.000Z' },
  ],
})
console.log(`  shape   ${realDocs.files.length} entities, synthetic=${realDocs.synthetic}`)
check('real documents', filemanager(realDocs.files))

// --- verdict --------------------------------------------------------------

console.log('')
if (failures.length > 0) {
  console.error(`FAILED: ${failures.length} case(s) — a widget rejected our data.`)
  for (const failure of failures) console.error(`  - ${failure}`)
  process.exit(1)
}
console.log('PASS: every wired widget accepted every projection shape.')


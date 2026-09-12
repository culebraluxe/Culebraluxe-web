import React from 'react'
import { renderToString } from 'react-dom/server'
import { Gantt } from '@svar-ui/react-gantt'

import { mapProjectToTimeline } from '../ui/projects/timeline-projection'
import type { ProjectPlan, ProjectWorkNode } from '../ui/projects/model'

// ---------------------------------------------------------------------------
// Headless check: does the SVAR Gantt actually ACCEPT what our projection feeds
// it?    pnpm check:gantt
//
// Why this exists: the Timeline pane shipped once and blew up in the browser with
// "null is not an object (evaluating 'e.forEach')". The unit tests could not catch
// that — the projection was internally consistent, and the crash lived inside the
// widget's own store, on a task field COMBINATION (`open: true` on a LEAF, which
// the store clears to `data: null` and then recurses into without a null guard).
//
// The widget SSR-renders, so this runs in a second with no browser and no DB.
// Extend it for each new widget: render it with the real projection's output.
//
// Note: the SVAR packages' CJS `main` points at `dist/index.cjs.js`, which is not
// shipped (the file is `dist/index.cjs`), so a plain require fails. This file is
// .mts so it loads the ESM build — the same one Next uses.
// ---------------------------------------------------------------------------

const failures: string[] = []

function attempt(name: string, tasks: unknown[], links: unknown[]): string | null {
  try {
    const html = renderToString(
      React.createElement(Gantt as never, { tasks, links, readonly: true } as never),
    )
    console.log(`  OK      ${name} (${html.length} chars)`)
    return null
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    console.log(`  THREW   ${name}: ${message}`)
    failures.push(`${name}: ${message}`)
    return message
  }
}

const node = (
  id: string,
  title: string,
  overrides: Partial<ProjectWorkNode> = {},
): ProjectWorkNode => ({ id, title, type: 'task', status: 'not-started', ...overrides })

// The shape that crashed: real WBS items, no due dates, one branch, several leaves.
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

console.log('SVAR Gantt acceptance check\n')

// 1. Control: the vendor's own example. If THIS fails, the harness is wrong.
console.log('control')
attempt(
  'vendor example',
  [
    { id: 1, text: 'Project', type: 'summary', parent: 0, open: true },
    { id: 10, text: 'Task', type: 'task', parent: 1, start: new Date(2026, 3, 2), duration: 3, progress: 40 },
  ],
  [{ id: 1, source: 10, target: 11, type: 'e2s' }],
)

// 2. The real projection, undated (the sample-schedule path).
console.log('\nreal projection')
const sample = mapProjectToTimeline(project)
console.log(`  shape   ${sample.tasks.length} tasks, ${sample.links.length} links, synthetic=${sample.synthetic}`)
attempt('undated project (sample schedule)', sample.tasks, sample.links)

// 3. The real projection, dated.
console.log('\ndated projection')
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
attempt('dated project', dated.tasks, dated.links)

// 4. Negative control: the exact shape that crashed. If this ever STOPS throwing,
//    the vendor hardened the store and the open-on-branches-only rule in
//    timeline-projection can be revisited.
console.log('\nnegative control (the original defect)')
const stillThrows = attempt(
  'open on a LEAF task',
  [
    { id: 1, text: 'P', type: 'summary', parent: 0, open: true },
    { id: 2, text: 'Leaf', type: 'task', parent: 1, start: new Date(2026, 3, 2), duration: 3, open: true },
  ],
  [],
)
if (stillThrows) {
  failures.pop() // expected
  console.log('  (expected — this is why leaf tasks are never marked open)')
} else {
  console.log('  NOTE: the vendor no longer throws here; the open-leaf rule may be relaxable.')
}

console.log('')
if (failures.length > 0) {
  console.error(`FAILED: ${failures.length} case(s) — the widget rejected our data.`)
  for (const failure of failures) console.error(`  - ${failure}`)
  process.exit(1)
}
console.log('PASS: the Gantt accepted every projection shape.')

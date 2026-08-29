import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { OPERATING_SURFACES, surfaceForPathname } from '../../lib/navigation/registry'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-NAV — discoverability wiring proof (source-based, matching the
// repo's existing tech-engineering-cockpit convention). Verifies the four-view
// console is the primary reachable destination and Runtime Inspector is clearly
// secondary, with no duplicate global nav and a single canonical API.
// ---------------------------------------------------------------------------

const read = (p: string) => readFile(new URL(p, import.meta.url), 'utf8')

test('FLIGHT-RECORDER-NAV 1: TECH registry has exactly one Flight Recorder destination', () => {
  const fr = OPERATING_SURFACES.TECH.items.filter((i) => i.label === 'Flight Recorder')
  assert.equal(fr.length, 1, 'exactly one Flight Recorder nav item')
  assert.equal(fr[0].href, '/portal/tech/flight-recorder')
})

test('FLIGHT-RECORDER-NAV 2: the nested console route is owned by TECH', () => {
  assert.equal(surfaceForPathname('/portal/tech/flight-recorder/abc-123'), 'TECH')
})

test('FLIGHT-RECORDER-NAV 3/4: Golden QA and normal executions open the console route', async () => {
  const src = await read('../../components/portal/tech/flight-recorder-list.tsx')
  // Golden QA primary action routes to the four-view console.
  assert.match(src, /\/portal\/tech\/flight-recorder\/\$\{goldenQa\.instanceId\}/)
  // Normal execution primary action routes to the four-view console.
  assert.match(src, /\/portal\/tech\/flight-recorder\/\$\{s\.instanceId\}/)
  // "Open Flight Recorder" is the primary wording.
  assert.ok(src.includes('Open Flight Recorder'))
})

test('FLIGHT-RECORDER-NAV 5: Runtime Inspector remains a clearly secondary action', async () => {
  const src = await read('../../components/portal/tech/flight-recorder-list.tsx')
  assert.match(src, /\/portal\/runtime-inspector\/\$\{s\.instanceId\}/)
  assert.ok(src.includes('Runtime Inspector'))
})

test('FLIGHT-RECORDER-NAV 6/8: console renders the four local view tabs, Timeline default', async () => {
  const page = await read('../../components/portal/tech/flight-recorder-console/FlightRecorderPage.tsx')
  const state = await read('../../components/portal/tech/flight-recorder-console/useFlightRecorderState.ts')
  for (const label of ['Timeline', 'Causality Graph', 'System Swimlane', 'Raw Events']) {
    assert.ok(page.includes(label), `tab "${label}" present`)
  }
  assert.ok(state.includes("'timeline'"), 'Timeline is the default view')
})

test('FLIGHT-RECORDER-NAV 7: the console uses a single canonical Flight Recorder API', async () => {
  const shell = await read('../../components/portal/tech/flight-recorder-console-shell.tsx')
  assert.match(shell, /\/api\/portal\/flight-recorder\/\$\{instanceId\}/)
  assert.ok(!shell.includes('/api/portal/runtime-inspector'), 'no Runtime Inspector API in the primary console path')
})

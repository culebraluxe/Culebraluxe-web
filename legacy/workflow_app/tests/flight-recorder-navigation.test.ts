import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { OPERATING_SURFACES, surfaceForPathname } from '@/lib/navigation/registry'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-NAV — discoverability wiring proof (source-based, matching the
// repo's existing tech-engineering-cockpit convention). Verifies the four-view
// console is the primary reachable destination and Runtime Inspector is clearly
// secondary, with no duplicate global nav and a single canonical API.
// ---------------------------------------------------------------------------

const read = (p: string) => readFile(new URL(p, import.meta.url), 'utf8')

test('FLIGHT-RECORDER-NAV 1: no surface carries a Flight Recorder LIST (retired 2026-09-13)', () => {
  // b3a6367 removed the list destinations from the TECH nav (Command Center, Command Console,
  // GROK, Flight Recorder) while keeping their routes. The console's door is the story cockpit
  // now, for stories the engine has actually executed; a nav item listing every instance would be
  // a second door to one trace, and the nav was the place operators read as "the machine".
  //
  // This test used to assert the opposite (`=== 1` + href `/portal/tech/flight-recorder`) and went
  // on asserting a retired destination for hours, because the flight-recorder suite is not in the
  // `forge-*` glob. It now asserts the retirement itself, so re-adding the nav item fails here.
  const surfaces = Object.values(OPERATING_SURFACES) as unknown as Array<{
    items: Array<{ label: string; href: string }>
  }>
  for (const surface of surfaces) {
    const fr = surface.items.filter((i) => i.label === 'Flight Recorder')
    assert.equal(fr.length, 0, 'the Flight Recorder list must not be a nav destination')
  }
})

test('FLIGHT-RECORDER-NAV 1b: the story cockpit is the door to the console', async () => {
  const src = await read('../../../components/portal/tech/engineering-cockpit.tsx')
  // The cockpit passes the story's latest instance, and renders the link only when one exists,
  // so an operator reaches a real trace instead of an empty shell.
  assert.match(src, /\/portal\/tech\/flight-recorder\/\$\{recorderInstanceId\}/)
})

test('FLIGHT-RECORDER-NAV 2: the nested console route is owned by TECH', () => {
  assert.equal(surfaceForPathname('/portal/tech/flight-recorder/abc-123'), 'TECH')
})

test('FLIGHT-RECORDER-NAV 3/4: Golden QA and normal executions open the console route', async () => {
  const src = await read('../../../components/portal/tech/flight-recorder-list.tsx')
  // Golden QA primary action routes to the four-view console.
  assert.match(src, /\/portal\/tech\/flight-recorder\/\$\{goldenQa\.instanceId\}/)
  // Normal execution primary action routes to the four-view console.
  assert.match(src, /\/portal\/tech\/flight-recorder\/\$\{s\.instanceId\}/)
  // "Open Flight Recorder" is the primary wording.
  assert.ok(src.includes('Open Flight Recorder'))
})

test('FLIGHT-RECORDER-NAV 5: Runtime Inspector remains a clearly secondary action', async () => {
  const src = await read('../../../components/portal/tech/flight-recorder-list.tsx')
  assert.match(src, /\/portal\/runtime-inspector\/\$\{s\.instanceId\}/)
  assert.ok(src.includes('Runtime Inspector'))
})

test('FLIGHT-RECORDER-NAV 6/8: console renders the four local view tabs, Timeline default', async () => {
  const page = await read('../../../components/portal/tech/flight-recorder-console/FlightRecorderPage.tsx')
  const state = await read('../../../components/portal/tech/flight-recorder-console/useFlightRecorderState.ts')
  for (const label of ['Timeline', 'Causality Graph', 'System Swimlane', 'Raw Events']) {
    assert.ok(page.includes(label), `tab "${label}" present`)
  }
  assert.ok(state.includes("'timeline'"), 'Timeline is the default view')
})

test('FLIGHT-RECORDER-NAV 7: the console uses a single canonical Flight Recorder API', async () => {
  const shell = await read('../../../components/portal/tech/flight-recorder-console-shell.tsx')
  assert.match(shell, /\/api\/portal\/flight-recorder\/\$\{instanceId\}/)
  assert.ok(!shell.includes('/api/portal/runtime-inspector'), 'no Runtime Inspector API in the primary console path')
})

test('FLIGHT-RECORDER-NAV 9: the Yew island preserves Next App Router context', async () => {
  const island = await read('../../../components/rust-ui/flight-recorder-react-island.tsx')
  assert.ok(island.includes("createPortal"), 'the console must stay in the parent React/Next context')
  assert.ok(!island.includes("createRoot("), 'a detached React root loses the Next App Router provider')
})

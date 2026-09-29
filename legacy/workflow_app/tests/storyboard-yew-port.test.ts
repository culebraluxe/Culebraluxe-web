import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFile } from 'node:fs/promises'

const read = (path: string) => readFile(new URL(path, import.meta.url), 'utf8')

test('STORYBOARD YEW: route mounts the Portal Yew app directly', async () => {
  const page = await read('../../../app/portal/storyboard/page.tsx')
  assert.match(page, /PortalYewApp/)
  assert.match(page, /screen="storyboard"/)
  assert.ok(!page.includes('RustUiHost'), 'Story Board must not fall back to the legacy generic host')
})

test('STORYBOARD YEW: canonical TypeScript cockpit projection is the read seam', async () => {
  const route = await read('../../../app/api/portal/rust-ui/page/route.ts')
  assert.match(route, /case 'storyboard'/)
  assert.match(route, /listStoryboardStories/)
  assert.match(route, /buildStoryBoardModel/)
  assert.match(route, /buildStoryBoardCockpit/)
  assert.match(route, /open: panel\('open'\)/)
  assert.match(route, /backlog: panel\('backlog'\)/)
  assert.match(route, /closed: panel\('closed'\)/)
  assert.match(route, /nextVersion: panel\('next-version'\)/)
})

test('STORYBOARD YEW: native view renders the six KPIs and four lifecycle panels', async () => {
  const view = await read('../../../rust/ui/src/yew_views/portal_storyboard.rs')
  for (const label of [
    'Total stories',
    'Open',
    'Backlog',
    'Blocked / Hold',
    'Complete',
    'Completion',
    'Current work queue',
    'Current-version waiting',
    'Finished history',
    'Intentionally future',
    'Next Version',
  ]) {
    assert.ok(view.includes(label), `${label} is rendered by Yew`)
  }
  assert.match(view, /PortalStoryboardPage/)
  assert.match(view, /\/portal\/storyboard\//)
  assert.ok(!view.includes('<button'), 'read-only cockpit has no command buttons')
  assert.ok(!view.includes('onclick='), 'read-only cockpit has no write click handlers')
})

test('STORYBOARD YEW: typed portal fetch and runtime mount admission agree', async () => {
  const update = await read('../../../rust/ui/src/update.rs')
  const portal = await read('../../../rust/ui/src/yew_portal.rs')
  const model = await read('../../../rust/ui/src/model.rs')

  assert.match(update, /"storyboard"/)
  assert.match(update, /pub fn has_yew_portal_component/)
  assert.match(portal, /has_yew_portal_component\(screen\.key\)/)
  assert.match(model, /pub storyboard: Option<PortalStoryboardPage>/)
  assert.match(model, /pub struct PortalStoryboardPanels/)
})

test('STORYBOARD YEW: no React rendering island is required', async () => {
  const host = await read('../../../components/rust-ui/portal-yew-app.tsx')
  assert.ok(
    !host.includes("screen === 'storyboard' ?"),
    'Story Board is rendered natively by Yew, not a React island',
  )
})

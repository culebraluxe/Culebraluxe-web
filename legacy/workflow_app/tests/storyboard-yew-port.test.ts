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

test('STORYBOARD YEW: snapshot is native Yew with no React island or write controls', async () => {
  const view = await read('../../../rust/ui/src/yew_views/portal_storyboard.rs')
  const host = await read('../../../components/rust-ui/portal-yew-app.tsx')

  assert.match(view, /PortalShell/)
  assert.match(view, /Authoritative backlog/)
  assert.match(view, /Completion/)
  assert.match(view, /\/portal\/storyboard\//)
  assert.ok(!view.includes('<button'), 'snapshot has no command buttons')
  assert.ok(!view.includes('onclick='), 'snapshot has no write/control click handlers')

  assert.ok(
    !host.includes("screen === 'storyboard' ?"),
    'Story Board must not require a React rendering island',
  )
})

test('STORYBOARD YEW: row contract includes stored completion', async () => {
  const route = await read('../../../app/api/portal/rust-ui/rows/route.ts')
  const start = route.indexOf('function storyboardRows')
  assert.notEqual(start, -1)
  const body = route.slice(start, start + 900)
  assert.match(body, /story\.completion/)
  assert.match(body, /badge: story\.status/)
})

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFile } from 'node:fs/promises'

const read = (path: string) => readFile(new URL(path, import.meta.url), 'utf8')

test('UI LAB: Yew owns the route and exposes bounded React island slots', async () => {
  const yew = await read('../../../rust/ui/src/yew_views/portal_ui_lab.rs')
  assert.match(yew, /id="ui-lab-react-island"/)
  assert.match(yew, /id="ui-lab-motion-island"/)
  assert.ok(!yew.includes('Open preserved Framer lab'), 'Framer is embedded, not a second nav destination')
})

test('UI LAB: the portal host mounts preserved galleries without a detached React root', async () => {
  const host = await read('../../../components/rust-ui/portal-yew-app.tsx')
  const islands = await read('../../../components/rust-ui/ui-lab-react-islands.tsx')

  assert.match(host, /screen === 'design-lab'.*UiLabReactIslands/s)
  assert.match(islands, /createPortal/)
  assert.ok(!islands.includes('createRoot('), 'bounded islands must retain the parent React/Next context')
  assert.match(islands, /TypeScriptUiLab/)
  assert.match(islands, /FramerUiLab/)
})

test('UI LAB: the original TypeScript gallery remains a real component reference', async () => {
  const gallery = await read('../../../components/portal/tech/typescript-ui-lab.tsx')
  for (const primitive of [
    'PageHeader',
    'Panel',
    'PortalCombobox',
    'PortalDialog',
    'PortalPagination',
    'ActivityTimeline',
    'ProcessSteps',
  ]) {
    assert.ok(gallery.includes(primitive), `${primitive} remains represented`)
  }
})

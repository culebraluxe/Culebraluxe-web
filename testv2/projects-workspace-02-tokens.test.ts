import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  PROJECTS_PANE_ORDER,
  PROJECTS_GEOMETRY,
  PROJECTS_GRID_TEMPLATE,
  PROJECTS_SURFACE,
  PROJECTS_PRIMITIVES,
  PROJECTS_LONG_CONTENT,
} from '../ui/projects/visual-system'

// PROJECTS-WORKSPACE-02 — the frozen three-pane geometry + visual system.
// This machine assay cannot render React (repo is node:test + tsx only), so the
// visual contract lives in a pure, React-free module. These tests lock the
// approved desktop geometry (navigator -> canvas -> inspector), the per-role
// surface families, the row/input/button/avatar primitive builders, and the
// long-content policy. Pixel balance is the separate HUMAN GATE.

test('the three panes are ordered navigator -> canvas -> inspector', () => {
  assert.deepEqual([...PROJECTS_PANE_ORDER], ['navigator', 'canvas', 'inspector'])
})

test('canvas is the sole flexible column and navigator/inspector are bounded', () => {
  assert.equal(PROJECTS_GEOMETRY.columns.canvas.track, 'minmax(0,1fr)')
  assert.equal(PROJECTS_GEOMETRY.columns.canvas.max, null)

  assert.ok(PROJECTS_GEOMETRY.columns.navigator.min > 0)
  assert.ok(PROJECTS_GEOMETRY.columns.navigator.max !== null)
  assert.ok(PROJECTS_GEOMETRY.columns.inspector.min > 0)
  assert.ok(PROJECTS_GEOMETRY.columns.inspector.max !== null)
})

test('the grid template follows pane order with canvas in the middle', () => {
  const tracks = PROJECTS_GRID_TEMPLATE.split(' ')
  assert.deepEqual(tracks, [
    PROJECTS_GEOMETRY.columns.navigator.track,
    PROJECTS_GEOMETRY.columns.canvas.track,
    PROJECTS_GEOMETRY.columns.inspector.track,
  ])
  assert.equal(tracks[1], 'minmax(0,1fr)')
})

test('the viewport strategy is shell-owned, not a hardcoded magic number', () => {
  assert.equal(PROJECTS_GEOMETRY.viewport.chromeHeightVar, '--portal-shell-chrome-height')
  assert.equal(PROJECTS_GEOMETRY.breakpoint, 'lg')
  assert.ok(PROJECTS_GEOMETRY.gridClassName.includes('projects-workspace-grid'))
})

test('each role has its own surface family with no cross-contamination', () => {
  assert.equal(PROJECTS_SURFACE.navigator.family, 'navy')
  assert.equal(PROJECTS_SURFACE.canvas.family, 'canvas')
  assert.equal(PROJECTS_SURFACE.inspector.family, 'ivory')

  const families = new Set(Object.values(PROJECTS_SURFACE).map((s) => s.family))
  assert.equal(families.size, 3)

  assert.ok(PROJECTS_SURFACE.navigator.background.includes('--portal-navy'))
  assert.ok(PROJECTS_SURFACE.inspector.background.includes('--portal-ivory'))
  assert.ok(PROJECTS_SURFACE.canvas.background.includes('--portal-panel-bg'))
})

test('the surface tokens mirror the CSS rules that actually paint the panes', () => {
  // The token object is documentation + contract; app/globals.css is the paint.
  // Those two drifted once and it shipped: the canvas token was moved to navy and
  // this suite was updated to match, while `.projects-pane-canvas` kept painting
  // light glass — so all the dark-on-navy classes inside the canvas became
  // invisible, and the view nav vanished. This assertion keeps them in step: the
  // CSS rule for each role must carry exactly the background/color the token
  // declares, so moving a surface means editing both or failing here.
  const css = readFileSync(new URL('../app/globals.css', import.meta.url), 'utf8')

  for (const role of PROJECTS_PANE_ORDER) {
    const surface = PROJECTS_SURFACE[role]
    const rule = css.match(
      new RegExp(`\\.projects-pane\\.projects-pane-${role}\\s*\\{([^}]*)\\}`),
    )
    assert.ok(rule, `no .projects-pane-${role} rule found in globals.css`)

    const body = rule[1].replace(/\s+/g, ' ').trim()
    assert.ok(
      body.includes(`background-color: ${surface.background};`),
      `.projects-pane-${role} background drifted from the token: expected "${surface.background}" in "${body}"`,
    )
    assert.ok(
      body.includes(`color: ${surface.text};`),
      `.projects-pane-${role} text color drifted from the token: expected "${surface.text}" in "${body}"`,
    )
  }
})

test('surface classes never leak one role token into another role', () => {
  assert.ok(PROJECTS_SURFACE.navigator.className.includes('projects-pane-navigator'))
  assert.ok(PROJECTS_SURFACE.canvas.className.includes('projects-pane-canvas'))
  assert.ok(PROJECTS_SURFACE.inspector.className.includes('projects-pane-inspector'))

  assert.ok(!PROJECTS_SURFACE.navigator.className.includes('projects-pane-inspector'))
  assert.ok(!PROJECTS_SURFACE.inspector.className.includes('projects-pane-navigator'))
})

test('primitive class builders return token-driven, non-empty classes', () => {
  const row = PROJECTS_PRIMITIVES.row()
  const rowSelected = PROJECTS_PRIMITIVES.row({ selected: true })
  assert.ok(row.length > 0)
  assert.ok(row.includes('rounded-[var(--portal-tab-radius)]'))
  assert.notEqual(row, rowSelected)

  // Selected state is SURFACE-AWARE: a navy tint is invisible on the navy canvas,
  // so the midnight variant lifts with white instead.
  const navySelected = PROJECTS_PRIMITIVES.row({ selected: true, surface: 'navy' })
  assert.notEqual(navySelected, rowSelected)
  assert.ok(navySelected.includes('bg-white/12'))
  assert.ok(!navySelected.includes('--portal-navy)/[0.06]'))

  const input = PROJECTS_PRIMITIVES.input()
  const navyInput = PROJECTS_PRIMITIVES.input({ surface: 'navy' })
  assert.ok(input.length > 0)
  assert.ok(navyInput.includes('text-white'))

  const primary = PROJECTS_PRIMITIVES.button()
  const ghost = PROJECTS_PRIMITIVES.button({ variant: 'ghost' })
  assert.ok(primary.includes('--portal-navy'))
  assert.notEqual(primary, ghost)

  const avatar = PROJECTS_PRIMITIVES.avatar()
  const largeAvatar = PROJECTS_PRIMITIVES.avatar({ size: 'lg' })
  assert.ok(avatar.includes('--portal-gold'))
  assert.notEqual(avatar, largeAvatar)
})

test('long-content policy: labels truncate, prose wraps, lists scroll', () => {
  assert.equal(PROJECTS_LONG_CONTENT.label.policy, 'truncate')
  assert.ok(PROJECTS_LONG_CONTENT.label.classes.includes('truncate'))

  assert.equal(PROJECTS_LONG_CONTENT.prose.policy, 'wrap')
  assert.ok(PROJECTS_LONG_CONTENT.prose.classes.includes('break-words'))

  assert.equal(PROJECTS_LONG_CONTENT.list.policy, 'scroll')
  assert.ok(PROJECTS_LONG_CONTENT.list.classes.includes('overflow-y-auto'))
  assert.ok(PROJECTS_LONG_CONTENT.list.classes.includes('min-h-0'))
})

test('the visual-system module imports no React and touches no DOM globals', () => {
  const source = readFileSync(new URL('../ui/projects/visual-system.ts', import.meta.url), 'utf8')
  assert.ok(!/\bfrom\s+['"]react['"]/.test(source))
  assert.ok(!/\brequire\(\s*['"]react['"]\s*\)/.test(source))
  assert.ok(!/\bdocument\s*\./.test(source))
  assert.ok(!/\bwindow\s*\./.test(source))
})

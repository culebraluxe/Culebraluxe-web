import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

// ---------------------------------------------------------------------------
// COMMAND + STATUS BAND — reusable design-system component.
// Source/shape guards (the component owns layout + responsive + status visuals).
// ---------------------------------------------------------------------------

test('command-status-band: reusable component exists with ratio presets', () => {
  const src = readFileSync(
    new URL('../../components/portal/command-status-band.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/export function CommandStatusBand/.test(src), 'CommandStatusBand exported')
  assert.ok(/export function CommandStatus/.test(src), 'CommandStatus exported')
  assert.ok(/wide-command/.test(src), 'wide-command preset')
  assert.ok(/balanced/.test(src), 'balanced preset')
  assert.ok(/wide-status/.test(src), 'wide-status preset')
  // Responsive: stacked on mobile (grid-cols-1), ratio on desktop (lg:grid-cols).
  assert.ok(/grid-cols-1/.test(src), 'stacks on narrow screens')
  assert.ok(/lg:grid-cols-/.test(src), 'side-by-side on desktop/tablet')
  // Accessibility: status not colour-only, aria-live status text.
  assert.ok(/aria-live/.test(src), 'status text announces updates')
  assert.ok(/aria-hidden/.test(src), 'status dot hidden from AT (text carries meaning)')
})

test('command-status-band: Forms uses the reusable component (visually-neutral refactor)', () => {
  const src = readFileSync(
    new URL('../../components/portal/forms/form-editor.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/<CommandStatusBand/.test(src), 'FormEditor uses CommandStatusBand')
  assert.ok(/<CommandStatus/.test(src), 'FormEditor uses CommandStatus')
  assert.ok(/FormGrokHelper/.test(src), 'the Grok helper is the command slot')
  // No duplicated Forms-only status panel CSS remains.
  assert.ok(
    !/aria-label="Form status"/.test(src),
    'inline status panel replaced by the reusable component',
  )
})

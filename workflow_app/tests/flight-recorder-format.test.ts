import test from 'node:test'
import assert from 'node:assert/strict'

import { formatDisplayTime } from '../../components/portal/tech/flight-recorder-console/format'
import { formatDisplayTime as grokFormatDisplayTime } from '../../components/portal/tech/grok-flight-recorder/format'

// FLIGHT-RECORDER-FORMAT — the timestamp formatter shown in the console's
// Business Context. It MUST be deterministic (no Intl.DateTimeFormat locale
// output) so the SSR server and the browser produce byte-identical text —
// otherwise Next.js throws a hydration mismatch ("May 20, 2025 at ... AM"
// vs "May 20, 2025, ... AM").

test('formatDisplayTime renders a stable UTC string (no locale-dependent connector)', () => {
  const out = formatDisplayTime('2025-05-20T14:14:32.145Z')
  assert.equal(out, 'May 20, 2025 at 02:14:32.145 PM')
  // Deterministic: same input → same output, and it never embeds the ICU "at"
  // vs "," ambiguity that differs between Node and browser ICU.
  assert.equal(formatDisplayTime('2025-05-20T14:14:32.145Z'), out)
})

test('both consoles share the same deterministic timestamp format', () => {
  const a = formatDisplayTime('2025-05-20T14:14:32.145Z')
  const b = grokFormatDisplayTime('2025-05-20T14:14:32.145Z')
  assert.equal(a, b)
})

test('formatDisplayTime falls back to the raw input for an invalid date', () => {
  assert.equal(formatDisplayTime('not-a-date'), 'not-a-date')
})

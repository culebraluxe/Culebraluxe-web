import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  ALLOWLIST_ROOTS,
  checkTrailingWhitespace,
  formatViolation,
} from '../../scripts/check-trailing-whitespace'

// ---------------------------------------------------------------------------
// ENG-19-DOGFOOD-001 — repo invariant: no committed text file under the fixed
// allowlist (agent-runtime, db, docs/workflow, workflow_app, workflow_engine)
// may contain a line ending with a trailing space or tab.
//
// This test runs the same deterministic check the CLI performs against the
// real repository (committed files only, byte-level scan). Any finding fails
// the test with `path:line` diagnostics.
// ---------------------------------------------------------------------------

test('committed text files under the allowlist contain no trailing whitespace', () => {
  const result = checkTrailingWhitespace()

  assert.ok(
    result.filesScanned > 0,
    `expected the check to scan committed files under ${ALLOWLIST_ROOTS.join(', ')}`,
  )
  assert.deepEqual(
    result.violations.map(formatViolation),
    [],
    'trailing-whitespace violations (file:line)',
  )
})

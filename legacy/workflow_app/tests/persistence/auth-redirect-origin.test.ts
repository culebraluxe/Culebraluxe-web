import { test } from 'node:test'
import assert from 'node:assert/strict'

// ---------------------------------------------------------------------------
// AUTH-08D — DEV environment configures AUTH_URL to the canonical localhost
// origin. Lives in the persistence suite because it guards REAL environment
// configuration loaded via --env-file=.env.local (test:persistence), not a
// unit-level constant.
// ---------------------------------------------------------------------------

test('AUTH-08D: DEV environment configures AUTH_URL to the canonical localhost origin', () => {
  assert.equal(process.env.AUTH_URL, 'http://localhost:3000')
})

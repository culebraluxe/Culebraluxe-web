import { test } from 'node:test'
import assert from 'node:assert/strict'

// Placeholder so the canonical `pnpm test:persistence` glob resolves before the
// real app-side persistence proofs land (CRM-14B residential proof + ENG-09
// command-boundary proof live in this directory).
test('ENG-04: workflow engine schema is present in the DEV database', async () => {
  const { interactiveSql } = await import('../../../lib/neon-interactive')
  const rows = await interactiveSql`select to_regclass('process_definitions') as pd`
  assert.ok(rows[0]?.pd, 'process_definitions table must exist in DEV')
})

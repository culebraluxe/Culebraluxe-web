import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const scriptUrl = new URL('../../scripts/normalize-phone-identities.ts', import.meta.url)

test('phone Person consolidation preserves business state and fails closed', async () => {
  const source = await readFile(scriptUrl, 'utf8')

  assert.match(source, /when survivor\.role = 'unclassified' then loser\.role/)
  assert.match(source, /when survivor\.role in \('buyer', 'seller'\).*then 'both'/s)
  assert.match(source, /assigned_user_id = coalesce\(survivor\.assigned_user_id, loser\.assigned_user_id\)/)
  assert.match(source, /exception when unique_violation then\s+raise exception/s)
  assert.doesNotMatch(source, /exception when unique_violation then\s+null;/s)
})

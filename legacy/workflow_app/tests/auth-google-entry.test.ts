import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

test('fresh-browser Google entry stays on the proven Next/Auth.js server boundary', async () => {
  const source = await readFile(new URL('../../../app/login/page.tsx', import.meta.url), 'utf8')
  assert.match(source, /export const dynamic = "force-dynamic"/)
  assert.match(source, /<form\s+[\s\S]*?action=\{async \(\) => \{/)
  assert.match(source, /"use server"/)
  assert.match(source, /await signIn\("google", \{ redirectTo: "\/portal" \}\)/)
  assert.match(source, /AUTH_CONFIGURED/)
  assert.doesNotMatch(source, /RustUiHost|\/api\/auth\/signin\/google|skipCSRFCheck|redirect:\s*false/)
})

test('production auth proof cannot become a post-OAuth dead end', async () => {
  const source = await readFile(new URL('../../../app/portal-auth-proof/page.tsx', import.meta.url), 'utf8')
  assert.match(source, /if \(process\.env\.NODE_ENV === 'production'\) redirect\('\/portal'\)/)
  assert.match(source, /const session = await auth\(\)/)
  assert.doesNotMatch(source, /RustUiHost/)
})

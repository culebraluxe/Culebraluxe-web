import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

test('fresh-browser Google entry uses the existing Auth.js server action', async () => {
  const source = await readFile(new URL('../../../app/login/page.tsx', import.meta.url), 'utf8')
  assert.match(source, /<form action=\{async \(\) => \{/)
  assert.match(source, /'use server'/)
  assert.match(source, /await signIn\('google', \{ redirectTo: '\/portal-auth-proof' \}\)/)
  assert.match(source, /<RustUiHost rowsPath="\/api\/rust-ui\/public-rows" start="login" \/>/)
  assert.doesNotMatch(source, /skipCSRFCheck|signIn\('google',\s*\{\s*redirect:\s*false/)
})

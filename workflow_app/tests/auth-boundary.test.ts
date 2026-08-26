// ---------------------------------------------------------------------------
// AUTH-BOUNDARY — public site must have ZERO Portal-auth dependency.
//
// Structural guarantees:
//   1. PUBLIC route/page modules never IMPORT the Portal auth instance
//      (@/auth), the bare NextAuth package, or any @/lib/auth Portal
//      security/acting-user module. Only the JWT decoder (next-auth/jwt) is
//      permitted on a public module.
//   2. The public media route uses only the JWT decoder, never constructs
//      Auth.js (no @/lib/auth import, no bare next-auth).
//   3. Middleware matcher is narrow and explicit (/portal/:path*), never a
//      broad match-and-exclude over public routes.
//   4. The Portal has its own controlled error boundary (app/portal/error.tsx)
//      that is auth-free and presentation-only, so a Portal/auth failure is
//      contained and never surfaces on the public site.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const PUBLIC_MODULES = [
  'app/layout.tsx',
  'app/page.tsx',
  'app/buyers/page.tsx',
  'app/sellers/page.tsx',
  'app/about/page.tsx',
  'app/contact/page.tsx',
  'app/properties/[slug]/page.tsx',
  'app/api/media/[id]/route.ts',
  'app/api/media/documents/[id]/route.ts',
]

async function read(rel: string): Promise<string> {
  return readFile(new URL(`../../${rel}`, import.meta.url), 'utf8')
}

/** Static + dynamic import specifiers in a source file. */
function importSpecifiers(source: string): string[] {
  const out: string[] = []
  for (const line of source.split('\n')) {
    const m = line.match(/(?:from\s+)?["']([^"']+)["']/)
    if (m && /^\s*import\b/.test(line)) out.push(m[1])
    const d = line.match(/import\(\s*["']([^"']+)["']\s*\)/)
    if (d) out.push(d[1])
  }
  return out
}

/** A Portal-auth dependency: the Auth.js instance, bare next-auth, or any
 *  @/lib/auth module. The JWT decoder (next-auth/jwt) is explicitly allowed. */
function isForbiddenImport(mod: string): boolean {
  if (mod === '@/auth') return true
  if (mod.startsWith('@/lib/auth/')) return true
  if (mod === 'next-auth') return true
  if (mod.startsWith('next-auth/') && !mod.startsWith('next-auth/jwt')) return true
  return false
}

test('AUTH-BOUNDARY: every public route/page module never imports Portal auth', async () => {
  for (const rel of PUBLIC_MODULES) {
    const source = await read(rel)
    const forbidden = importSpecifiers(source).filter(isForbiddenImport)
    assert.deepEqual(
      forbidden,
      [],
      `${rel} must not import Portal auth (got: ${forbidden.join(', ') || 'none'})`,
    )
  }
})

test('AUTH-BOUNDARY: public media route uses only the JWT decoder, never Auth.js', async () => {
  const source = await read('app/api/media/[id]/route.ts')
  const specs = importSpecifiers(source)
  assert.ok(
    specs.includes('next-auth/jwt'),
    'media route imports the JWT decoder (next-auth/jwt)',
  )
  assert.deepEqual(
    specs.filter(isForbiddenImport),
    [],
    'media route must not import the Auth.js instance or Portal auth modules',
  )
})

test('AUTH-BOUNDARY: root layout has no Portal-auth dependency', async () => {
  const source = await read('app/layout.tsx')
  assert.deepEqual(
    importSpecifiers(source).filter(isForbiddenImport),
    [],
    'root layout must not import Portal auth',
  )
})

test('AUTH-BOUNDARY: middleware matcher is narrow and explicit (/portal/:path*)', async () => {
  const source = await read('middleware.ts')
  assert.ok(
    source.includes("matcher: ['/portal/:path*']"),
    'middleware matcher is scoped to /portal/:path*',
  )
  // No broad Next.js negate/exclude pattern and no catch-all root matcher.
  assert.ok(!source.includes("'/((?"), 'no broad match-and-exclude over public routes')
  assert.ok(!source.includes("matcher: ['/']"), 'no catch-all root matcher')
})

test('AUTH-BOUNDARY: Portal has its own controlled error boundary (auth-free)', async () => {
  const source = await read('app/portal/error.tsx')
  assert.ok(source.includes("'use client'"), 'portal error boundary is a client component')
  assert.ok(
    source.includes('Portal temporarily unavailable'),
    'controlled Portal-unavailable messaging',
  )
  assert.deepEqual(
    importSpecifiers(source).filter(isForbiddenImport),
    [],
    'portal error boundary must not import Portal auth',
  )
})


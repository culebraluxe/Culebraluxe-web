// AUTH-02 Portal route enforcement — cheap Edge first gate.
//
// Applies PORTAL_ROUTE_POLICY (lib/auth/route-policy.ts) for a cheap redirect
// before any server component runs:
//   - unauthenticated /portal*                      → /login
//   - authenticated but missing the required        → /login/unauthorized
//     authority (from the JWT capability snapshot)
//
// This gate is intentionally NOT authoritative: the Edge runtime cannot
// reliably reach the Neon pool for the authority projection, so the snapshot is
// read from the Auth.js JWT (stamped at sign-in by the jwt callback, never
// re-resolved here). Sessions without a snapshot pass through — the Portal
// layout (server component) performs the authoritative getActingUser +
// requireAuthority check and redirects on AuthError. Enforcement is defense in
// depth: middleware (cheap) + server-side layout/actions (authoritative).
//
// The JWT is decrypted with next-auth/jwt getToken (jose) — no @/auth import,
// no database driver, no provider config in the Edge bundle.

import { NextResponse, type NextRequest } from 'next/server'
import { getToken } from 'next-auth/jwt'

import { decidePortalMiddleware } from '@/lib/auth/middleware-policy'

// Auth.js v5 default session cookie names (dev = un-prefixed, prod = secure).
const SESSION_COOKIE_NAMES = [
  '__Secure-authjs.session-token',
  'authjs.session-token',
] as const

type PortalToken = {
  sub?: string | null
  capabilities?: string[] | null
}

async function readPortalToken(req: NextRequest): Promise<PortalToken | null> {
  const cookieHeader = req.headers.get('cookie') ?? ''
  const cookieNames = new Set(
    cookieHeader
      .split(';')
      .map((part) => part.split('=')[0]?.trim())
      .filter(Boolean),
  )

  const secret = process.env.AUTH_SECRET ?? process.env.NEXTAUTH_SECRET
  if (!secret) return null

  for (const name of SESSION_COOKIE_NAMES) {
    if (!cookieNames.has(name)) continue
    const token = await getToken({
      req,
      cookieName: name,
      secureCookie: name.startsWith('__Secure-'),
      secret,
    })
    if (token) return token as PortalToken
  }
  return null
}

export default async function middleware(req: NextRequest) {
  // ═══════════════════════════════════════════════════════════════════════════
  // ⚠️ TEMP STARTUP AUTH BYPASS — REMOVE ME.  (startup dev flow only)
  // Skipping the /portal/:path* Edge gate while PORTAL_AUTH_BYPASS=1; the
  // server-side sibling in resolvePortalAccess covers the authoritative check.
  // NEVER set PORTAL_AUTH_BYPASS on production. Delete this block to restore
  // the real Edge gate.
  // ═══════════════════════════════════════════════════════════════════════════
  if (process.env.PORTAL_AUTH_BYPASS === '1') {
    return NextResponse.next()
  }

  const { pathname } = req.nextUrl

  const token = await readPortalToken(req)
  const decision = decidePortalMiddleware(pathname, {
    authenticated: Boolean(token?.sub),
    capabilities: token?.capabilities ?? null,
  })

  if (decision.kind === 'redirect') {
    return NextResponse.redirect(new URL(decision.to, req.url))
  }
  return NextResponse.next()
}

export const config = {
  matcher: ['/portal/:path*'],
}

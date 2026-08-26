// AUTH-01 — Auth.js (v5) instance for Portal authentication.
//
// Boundary: this module ONLY proves identity ("who"). It exposes the Auth.js
// primitives ({ handlers, auth, signIn, signOut }) and maps an authenticated
// provider to a stable provider subject. It never resolves roles/authorities —
// that is the application security layer's job (getActingUser → AUTH-02), and
// business services must never call `auth()` directly.
//
// Providers:
//   - Google (default) or a generic OIDC provider, configured exclusively from
//     environment (AUTH_PROVIDER / AUTH_GOOGLE_ID / AUTH_GOOGLE_SECRET /
//     AUTH_ISSUER).
//   - "break-glass" Credentials provider: verifies the recovery secret via the
//     canonical authenticateBreakGlass() and returns a stable subject derived
//     from the configured root app_user, so recovery sessions resolve through
//     the SAME application projection as normal logins.
//
// Session strategy: JWT (no Auth.js database adapter — the canonical identity
// mapping lives in auth_identity). The provider `sub` is the stable identity
// key; email is never used as an identity key.

import NextAuth from 'next-auth'
import Google from 'next-auth/providers/google'
import Credentials from 'next-auth/providers/credentials'
import type { DefaultSession } from 'next-auth'

import { authenticateBreakGlass } from '@/lib/auth/break-glass-authenticate'
import { getAuthProviderConfig } from '@/lib/auth/provider-config'
import { getSessionAuthoritySnapshot } from '@/lib/auth/session-capability-snapshot'

// Stable provider identifiers surfaced through the session adapter.
export const AUTH_PROVIDER_GOOGLE = 'google'
export const AUTH_PROVIDER_BREAK_GLASS = 'break-glass'

// JWT session expiry (Auth.js `session.maxAge`). Signed-out users clear the
// cookie via /api/auth/signout; the JWT simply stops being accepted after this.
export const SESSION_MAX_AGE_SECONDS = 60 * 60 * 24 * 7 // 7 days

// Stable subject for a break-glass session: derived from the configured root
// app_user id, never from email. Matches the auth_identity mapping created by
// db/manual/2026-08-20_v7_break_glass_identity.sql.
export function breakGlassSubject(appUserId: string): string {
  return `${AUTH_PROVIDER_BREAK_GLASS}:${appUserId}`
}

function buildProviders() {
  const config = getAuthProviderConfig()

  // AUTH-08 — fail-closed, explicit DEV diagnostic. When the default Google
  // provider lacks credentials, Auth.js would otherwise surface a generic
  // "server configuration" error at sign-in. Log the missing variable NAMES
  // (never values) so Chris knows exactly what to configure. Production still
  // fails closed at sign-in; this warning never fabricates credentials.
  if (config.provider === 'google' && (!config.clientId || !config.clientSecret)) {
    const missing = [
      !config.clientId ? 'AUTH_GOOGLE_ID' : null,
      !config.clientSecret ? 'AUTH_GOOGLE_SECRET' : null,
    ].filter(Boolean)
    console.warn(
      `[auth] Google OAuth unavailable: ${missing.join(' and ')} missing. ` +
        'Set them in .env.local (see docs/auth-google-setup.md). ' +
        'Normal Google login will fail closed until configured.',
    )
  }

  // Generic OIDC (AUTH_PROVIDER=oidc + AUTH_ISSUER) or Google (default).
  // Client credentials come from environment only. Auth.js's OAuth config
  // treats clientId/clientSecret/issuer as optional, so an unconfigured DEV
  // process constructs cleanly and fails closed at sign-in time.
  const oauthProvider =
    config.provider === 'oidc' && config.issuer
      ? {
          id: 'oidc',
          name: 'OpenID Connect',
          type: 'oidc' as const,
          issuer: config.issuer,
          clientId: config.clientId ?? undefined,
          clientSecret: config.clientSecret ?? undefined,
        }
      : Google({
          clientId: config.clientId ?? undefined,
          clientSecret: config.clientSecret ?? undefined,
        })

  return [
    oauthProvider,
    Credentials({
      id: AUTH_PROVIDER_BREAK_GLASS,
      name: 'Break-glass recovery',
      credentials: {
        secret: { label: 'Recovery credential', type: 'password' },
      },
      async authorize(credentials) {
        const secret = credentials?.secret
        if (typeof secret !== 'string' || secret.length === 0) return null
        const result = await authenticateBreakGlass(secret)
        if (!result.ok) return null
        // Stable subject for the configured root app_user. authenticateBreakGlass
        // already resolved the root through the NORMAL role/authority projection,
        // so the ActingUser shape is identical to a normal provider login.
        return {
          id: breakGlassSubject(result.actingUser.appUserId),
          name: result.actingUser.displayName,
          email: result.actingUser.email ?? undefined,
        }
      },
    }),
  ]
}

export const { handlers, auth, signIn, signOut } = NextAuth({
  providers: buildProviders(),
  session: {
    strategy: 'jwt',
    maxAge: SESSION_MAX_AGE_SECONDS,
  },
  callbacks: {
    async jwt({ token, user, account }) {
      if (user?.id) token.sub = user.id
      if (account?.provider) {
        token.provider = account.provider
      } else if (user?.id?.startsWith(`${AUTH_PROVIDER_BREAK_GLASS}:`)) {
        // Credentials sign-in may not carry an account object; derive the
        // provider deterministically from the stable subject prefix.
        token.provider = AUTH_PROVIDER_BREAK_GLASS
      }
      // AUTH-02: stamp the coarse authority snapshot for the Edge middleware
      // gate. Runs ONLY at sign-in (user present, Node runtime) — the snapshot
      // then rides in the JWT and is never re-resolved on subsequent requests.
      // The snapshot is not authoritative (server-side guards re-resolve from
      // the DB on every protected request), so a resolution failure degrades to
      // null: the cheap gate passes through and the layout guard decides.
      if (user?.id && token.provider) {
        token.capabilities = await getSessionAuthoritySnapshot(
          token.provider as string,
          token.sub as string,
        ).catch(() => null)
      }
      return token
    },
    session({ session, token }) {
      session.user.sub = (token.sub as string | undefined) ?? null
      session.user.provider = (token.provider as string | undefined) ?? null
      return session
    },
  },
})

declare module 'next-auth' {
  interface Session {
    user: {
      sub: string | null
      provider: string | null
    } & DefaultSession['user']
  }
}

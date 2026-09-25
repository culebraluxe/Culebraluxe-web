// AUTH-08F — minimal Auth.js v5 + Google baseline.
//
// This module ONLY proves identity ("who"): it authenticates via Google and
// exposes { handlers, auth, signIn, signOut }. It makes NO database calls, NO
// app_user lookup, NO role/authority snapshot, NO authorization inside Auth.js
// callbacks. Application authorization happens AFTER authentication, server-side
// in the Portal guard (getActingUser -> auth_identity -> app_user -> authorities).
//
// Provider: Google, configured exclusively from environment.
// Session strategy: JWT (no Auth.js database adapter). The provider `sub` is the
// stable identity key; email is never used as an identity key.
//
// Break-glass is TEMPORARILY kept out of this baseline (restored later as a
// separate provider path with authorization after authentication, same as
// Google). The pure helper exports below remain so the auth persistence tests
// still compile; no break-glass provider is registered.

import NextAuth from 'next-auth'
import Credentials from 'next-auth/providers/credentials'
import Google from 'next-auth/providers/google'
import type { DefaultSession } from 'next-auth'

import { devAuthLog } from '@/lib/auth/dev-auth-log'

// Stable provider identifiers surfaced through the session adapter.
export const AUTH_PROVIDER_GOOGLE = 'google'
export const AUTH_PROVIDER_BREAK_GLASS = 'break-glass'
// EXTERNAL GUESTS of the public website may also sign in with a code emailed to them. Rust checks the code and
// provisions the guest (security/guest.rs); the session's subject is the verified email. Guests are external
// accounts, which the Rust policy refuses every portal grant.
export const AUTH_PROVIDER_EMAIL_CODE = 'email-code'

// JWT session expiry. Signed-out users clear the cookie via /api/auth/signout.
export const SESSION_MAX_AGE_SECONDS = 60 * 60 * 24 * 7 // 7 days

// Stable subject for a future break-glass session (matches the auth_identity
// mapping in db/manual/2026-08-20_v7_break_glass_identity.sql). Not used while
// the break-glass provider is out of the baseline.
export function breakGlassSubject(appUserId: string): string {
  return `${AUTH_PROVIDER_BREAK_GLASS}:${appUserId}`
}

// (No buildProviders — baseline registers only Google directly below.)

export const { handlers, auth, signIn, signOut } = NextAuth({
  // Explicitly bind the env-backed Auth.js signing secret.
  secret: process.env.AUTH_SECRET,
  // TRUST THE REQUEST HOST — the fix for a production login that could not work.
  //
  // 2026-09-14: the portal's Google sign-in failed in production, sending the browser to
  // `localhost:3000`. Auth.js v5 builds the OAuth `redirect_uri` from a BASE URL, and when neither
  // `AUTH_URL` is set nor the host is trusted, that base URL is the development default
  // (`http://localhost:3000`). Production had `AUTH_SECRET`, `AUTH_GOOGLE_ID` and
  // `AUTH_GOOGLE_SECRET` and NOTHING pinning the URL - so Google was asked to return to a machine
  // that does not exist. Nothing in the build or deploy output hinted at it; the only symptom was a
  // browser error in a redirect chain, which is the worst place to find a configuration gap.
  //
  // `trustHost` makes Auth.js derive the URL from the request's forwarded host, which Vercel sets
  // correctly, so the redirect follows whichever canonical host the visitor is on instead of a
  // hardcoded dev fallback. `AUTH_URL` is also set in the Vercel project as an explicit canonical
  // (`https://www.culebraluxe.com`) - the belt to this pair of braces, because a pinned URL is
  // deterministic and a trusted host is forgiving.
  //
  // NOTE FOR WHOEVER CHANGES THE DOMAIN: the callback URI registered with Google is
  // `<canonical host>/api/auth/callback/google`, so the Vercel `AUTH_URL` and the Google console
  // entry must agree on the host (apex 308-redirects to www, so www is the one that matters).
  trustHost: true,
  providers: [
    Google({
      clientId: process.env.AUTH_GOOGLE_ID,
      clientSecret: process.env.AUTH_GOOGLE_SECRET,
    }),
    Credentials({
      id: AUTH_PROVIDER_EMAIL_CODE,
      credentials: { email: {}, code: {} },
      // Loaded lazily: the Rust client imports the session adapter, which imports this module.
      // A refused code (4xx) is the visitor's mistake: null sends them back to /account?error=CredentialsSignin. Any
      // other failure is captured durably and rethrown, so an outage is seen rather than shown as "wrong code".
      async authorize(credentials) {
        const [{ rustApiVerifyGuestCode, RustApiError }, { withServerErrorCapture }] = await Promise.all([
          import('@/lib/rust-api/client'),
          import('@/lib/error-capture-seam'),
        ])
        const verify = withServerErrorCapture('auth:email-code', async () => {
          try {
            return await rustApiVerifyGuestCode(String(credentials.email ?? ''), String(credentials.code ?? ''))
          } catch (error) {
            if (error instanceof RustApiError && error.status < 500) return null
            throw error
          }
        }, { route: '/account' })
        const verified = await verify()
        return verified ? { id: verified.email, email: verified.email } : null
      },
    }),
  ],
  session: { strategy: 'jwt', maxAge: SESSION_MAX_AGE_SECONDS },
  // Custom safe error page (DEV shows a diagnostic; PROD stays generic).
  // A refused email code returns to the guest sign-in screen (`/account?error=...`), not Auth.js's built-in page.
  pages: { error: '/auth/error', signIn: '/account' },
  callbacks: {
    // Minimal: stamp only the stable subject + provider. NO DB, NO authorities.
    //
    // Canonical identity key = account.providerAccountId (the provider's stable
    // account id, for Google the numeric `sub` from profile.sub). Do NOT use
    // `user.id`: in Auth.js v5 WITHOUT a database adapter, `user.id` is a fresh
    // crypto.randomUUID() minted per sign-in (see @auth/core oauth/callback),
    // so it is NOT stable. The provider subject is the durable identity key;
    // email is never an identity key.
    jwt({ token, account, profile }) {
      const stableSubject = account?.providerAccountId
      if (stableSubject) {
        token.sub = stableSubject
        devAuthLog('AUTH_GOOGLE_CALLBACK_RECEIVED')
        devAuthLog('AUTH_SESSION_CREATED')
      }
      if (account?.provider) {
        token.provider = account.provider
        // Rust links a guest's Google and email-code sign-ins only on a VERIFIED email.
        token.emailIsVerified = account.provider === AUTH_PROVIDER_EMAIL_CODE || profile?.email_verified === true
      }
      return token
    },
    session({ session, token }) {
      session.user.sub = (token.sub as string | undefined) ?? null
      session.user.provider = (token.provider as string | undefined) ?? null
      session.user.emailIsVerified = token.emailIsVerified === true
      return session
    },
  },
})

declare module 'next-auth' {
  interface Session {
    user: {
      sub: string | null
      provider: string | null
      emailIsVerified: boolean
    } & DefaultSession['user']
  }
}

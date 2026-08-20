# Auth.js Session Adapter — Import-Ready Implementation

The working tree holds a compile-safe stub at `lib/auth/authjs-session-adapter.ts`
because `next-auth` is not installed. After running `pnpm add next-auth@beta`,
replace the stub body with this implementation (do NOT fake sessions).

```ts
// lib/auth/authjs-session-adapter.ts
import { auth } from '@/auth'                 // Auth.js instance (see below)
import type { SessionAdapter } from './session-adapter'
import type { AuthenticatedIdentity } from './types'

export function createAuthJsSessionAdapter(): SessionAdapter {
  return {
    async getSession(): Promise<AuthenticatedIdentity | null> {
      const session = await auth()
      const user = session?.user
      if (!user?.sub) return null          // sub = stable provider subject
      return {
        provider: 'google',                // or from session/token
        providerSubject: user.sub,
        providerEmail: user.email ?? null,
      }
    },
  }
}
```

`@/auth` is the Auth.js v5 instance:

```ts
// auth.ts
import NextAuth from 'next-auth'
import Google from 'next-auth/providers/google'

export const { handlers, auth, signIn, signOut } = NextAuth({
  providers: [
    Google({
      clientId: process.env.AUTH_GOOGLE_ID,
      clientSecret: process.env.AUTH_GOOGLE_SECRET,
    }),
  ],
  callbacks: {
    jwt({ token, user }) {
      if (user) token.sub = user.id
      return token
    },
    session({ session, token }) {
      session.user.sub = (token.sub as string | undefined) ?? null
      return session
    },
  },
  // The provider `sub` is the stable subject. Do NOT use email as identity.
})
```

`app/api/auth/[...nextauth]/route.ts` exports `handlers`.

The application must only ever consume `getActingUser(adapter)` — never the
`auth()` primitive in business services.

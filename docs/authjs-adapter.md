# Auth.js Session Adapter — Implementation Notes

Auth.js (next-auth v5 beta) is installed and wired. This document records the
implemented design and the two deviations from the original import-ready sketch.

## Implemented files

| File | Purpose |
|------|---------|
| `auth.ts` | Auth.js v5 instance: `{ handlers, auth, signIn, signOut }`, Google/OIDC + break-glass Credentials providers, JWT strategy. |
| `app/api/auth/[...nextauth]/route.ts` | Exports `handlers` (`GET`/`POST`). |
| `lib/auth/authjs-session-adapter.ts` | `createAuthJsSessionAdapter()` → `AuthenticatedIdentity` from the Auth.js session. |
| `app/login/recovery/actions.ts` | `breakGlassLoginAction` verifies the secret and establishes the Auth.js Credentials session via `signIn('break-glass', …)`, then audits success. |

## Adapter contract

```ts
createAuthJsSessionAdapter().getSession()
// → { provider: 'google' | 'break-glass', providerSubject: <stable sub>, providerEmail }
// → null when no valid session / no stable subject
```

The provider `sub` is the stable identity key — email is informational only.
`auth` is injectable (`deps.auth`) for targeted tests; production always uses
the `@/auth` instance. Cookies/tokens never leave Auth.js; business services
consume only `getActingUser(adapter)`.

## Session / identity lifecycle

- JWT strategy (no Auth.js database adapter — canonical identity lives in
  `auth_identity`). `session.maxAge = 7 days` (`SESSION_MAX_AGE_SECONDS`).
- The jwt callback stamps `token.sub` (provider subject) and `token.provider`
  (`account.provider`, with a deterministic fallback to `'break-glass'` when the
  subject carries the `break-glass:` prefix).
- `signOut` via `/api/auth/signout` clears the cookie; the portal header shows a
  static "Sign out" link (no protection activated in AUTH-01).
- Unmapped/inactive identities resolve to `UnmappedIdentityError` /
  `InactiveAccountError` in `resolveProviderSubject` — no account auto-creation,
  no email fallback.

## Deviation 1 — generic OIDC provider

`next-auth@5.0.0-beta.32` does not ship `next-auth/providers/oidc` (no
`oidc.js` in the package). The generic OIDC provider is therefore constructed
inline in `auth.ts` using Auth.js's native OAuth config shape
(`type: 'oidc'` + `issuer` + `clientId`/`clientSecret`), selected when
`AUTH_PROVIDER=oidc` and `AUTH_ISSUER` is set. `AUTH_PROVIDER` defaults to
`google` → the built-in Google provider. Credentials come from environment only
(`AUTH_GOOGLE_ID` / `AUTH_GOOGLE_SECRET`).

## Deviation 2 — provider comes from the session, not a constant

The adapter reads `provider` from `session.user.provider` (stamped by the jwt
callback) instead of hardcoding `'google'`, so normal (google/oidc) and
break-glass sessions flow through the identical adapter contract.

## Break-glass

The Credentials provider calls `authenticateBreakGlass(secret)` and returns a
stable subject `break-glass:<root app_user id>` (see `breakGlassSubject()`).
That subject is mapped in `auth_identity` by
`db/manual/2026-08-20_v7_break_glass_identity.sql`, so recovery sessions resolve
through the SAME canonical projection as normal logins.

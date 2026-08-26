# Google OAuth — Local + Production Configuration (AUTH-08)

The Portal uses Auth.js v5 with **Google** as the default provider. Google login is
configured **exclusively from environment variables** — no credentials are ever
committed, and production fails closed if they are missing.

## Required environment variables (names only)

```
AUTH_SECRET=<existing auth secret — already set>
AUTH_GOOGLE_ID=<google oauth web client id>
AUTH_GOOGLE_SECRET=<google oauth web client secret>
```

Optional: `AUTH_PROVIDER=google` (default) or `AUTH_PROVIDER=oidc` + `AUTH_ISSUER=…`.

When `AUTH_GOOGLE_ID` / `AUTH_GOOGLE_SECRET` are absent, the server logs a clear
diagnostic (`[auth] Google OAuth unavailable: AUTH_GOOGLE_ID missing …`) and Google
login **fails closed** (no synthetic bypass). This is expected until the values are
configured.

## Google Cloud OAuth Web Client — required local settings

1. Google Cloud Console → **APIs & Services → Credentials → Create Credentials →
   OAuth client ID**.
2. Application type: **Web application**.
3. **Authorized JavaScript origins:**
   - `http://localhost:3000`
4. **Authorized redirect URIs:**
   - `http://localhost:3000/api/auth/callback/google`
5. Create → copy the **Client ID** and **Client Secret**.
6. Set them in `.env.local`:
   ```
   AUTH_GOOGLE_ID=<client id>
   AUTH_GOOGLE_SECRET=<client secret>
   ```

## Test the normal login path

```sh
# Ensure DEV bypass is OFF for this test
# (set PORTAL_AUTH_BYPASS=0 in .env.local)
pnpm dev
```

Then:
1. Open `http://localhost:3000/portal` → redirected to Login.
2. **Continue with Google** → account chooser → select your authorized account.
3. Callback completes → Auth.js session created.
4. `(provider='google', sub)` resolves through `auth_identity` → `app_user`.

## Map your Google subject to your application user

Identity is keyed by the stable **provider subject** (`sub`), never by email guessing.
After the first successful Google callback, run the human-executable linker with your
Google `sub`:

```
db/manual/2026-08-20_v5_link_auth_identity.sql
```

Set `v_provider := 'google'`, `v_subject := <your Google sub>`, and the target
`app_user` email. The linker guards against mapping to a different user and is
idempotent.

## Production

Set the same variables in the Vercel production environment. Production **never**
honors the DEV bypass (`PORTAL_AUTH_BYPASS`) — see HARDEN-01/HARDEN-04. Keep
`AUTH_BREAK_GLASS_*` for the unlinked recovery path.

# Auth Identity Bootstrap Order

The deliberate sequence that avoids lockout and avoids activating protection
before both normal owner login and break-glass recovery are proven.

1. Migration 016 (`auth_identity`) is already live (identity_count = 0).
2. Install the auth package: `pnpm add next-auth@beta`.
3. Configure the provider (Google/OIDC) in the provider console; set
   `AUTH_SECRET`, `AUTH_GOOGLE_ID`, `AUTH_GOOGLE_SECRET` (and `AUTH_TRUST_HOST=true`).
4. Authenticate once through the provider to learn the **stable provider subject**
   (or obtain it deterministically from provider tooling). Never use email.
5. Human links the subject → an existing active internal app_user using
   `db/manual/2026-08-20_v5_link_auth_identity.sql`.
6. Human assigns the first owner role using
   `db/manual/2026-08-20_v4_owner_bootstrap.sql` (if not already assigned).
7. Verify normal provider login resolves an ActingUser with owner authorities.
8. Configure break-glass: run `node scripts/generate-break-glass-hash.mjs '<secret>'`,
   then set `AUTH_BREAK_GLASS_ENABLED=true`, `AUTH_BREAK_GLASS_APP_USER_ID` (the
   deliberately bootstrapped owner), and `AUTH_BREAK_GLASS_SECRET_HASH` (the hash).
   Run `db/manual/2026-08-20_v6_security_audit_event.sql` (migration 017) first.
9. Link the break-glass provider identity to the same root app_user using
   `db/manual/2026-08-20_v7_break_glass_identity.sql` (provider `break-glass`,
   subject derived as `break-glass:<app_user id>`), so recovery sessions resolve
   through the same canonical projection as normal logins.
10. Verify break-glass login via `/login/recovery`.
11. Only now activate Portal route enforcement and server-command authorization.

There is no window where route protection is active before both normal owner
login and recovery login are proven.

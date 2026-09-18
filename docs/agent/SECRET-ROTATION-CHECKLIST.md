# Credential rotation checklist — the 2026-09-03/04 checkpoint capture

**Why this exists.** gitleaks found, on 2026-09-18, that a plaintext backup of the environment file
(`.env.local.before-icloud-username-fix`) had been captured by **Cline checkpoint refs** — shadow refs
under `refs/cline/checkpoints/`, which snapshot *untracked* files, so no human ever staged it, and the
old `.gitignore` rule `.env*.local` could not match a name ending in `-fix`.

**What is already done** (so this is not an open wound):

- `gitleaks` runs on every push and in CI; `.gitleaksignore` records the known history as a receipt.
- `.gitignore` now says `.env*` with `!.env.example`, so no spelling of an env file can slip again.
- The nine checkpoint refs were deleted and `git gc --prune=now` dropped the blob (`297c48e2` is gone,
  `git fsck` clean). **It was never pushed** — no branch and no remote ref ever held it, and CI's own
  scan of the remote (2,103 commits) sees nothing.

**What is left is rotation**, and only the operator can do it. Rotation is what *ends* the risk; the
purge only removed the local copy, and anything already copied off this machine stays exposed until the
value is invalidated at the provider. This is a 30-minute job, not an emergency.

## Rotate (secret values that appeared in that file)

| Provider | Variable(s) | Where |
| --- | --- | --- |
| **Neon** | `DATABASE_URL`, `DATABASE_URL_DEV`, `DATABASE_URL_PROD`, `DATABASE_URL_UNPOOLED` | Neon console → role password |
| **Auth** | `AUTH_SECRET` (regenerate), `AUTH_GOOGLE_SECRET`, `AUTH_GOOGLE_ID` | Google Cloud console |
| **Auth break-glass** | `AUTH_BREAK_GLASS_SECRET_HASH` | regenerate from the new secret |
| **Meta / WhatsApp** | `WHATSAPP_ACCESS_TOKEN`, `WHATSAPP_APP_SECRET`, `WHATSAPP_VERIFY_TOKEN` | Meta app dashboard |
| **BoldSign** | `BOLDSIGN_API_KEY`, `BOLDSIGN_WEBHOOK_SECRET` | BoldSign dashboard |
| **Apple** | `ICLOUD_MAIL_APP_PASSWORD` | appleid.apple.com → app-specific passwords |
| **Mux** | `MUX_TOKEN_SECRET_DEV`, `MUX_TOKEN_SECRET_PROD`, `MUX_DATA_ENV_KEY_DEV`, `MUX_DATA_ENV_KEY_PROD` | Mux dashboard (rotate the token **pair**) |
| **xAI** | `XAI_API_KEY` | xAI console |
| **Portal** | `PORTAL_REVIEW_TOKEN` | regenerate; it is a shared link secret |

## Do NOT rotate (present in the file, but not secrets)

`APP_ENV`, `AUTH_URL`, `AUTH_TRUST_HOST`, `AUTH_PROVIDER`, `NEON_BRANCH`, `EMAIL_INTERNAL_ADDRESSES`,
`GOOGLE_MAPS_DEMO_KEY` (demo keys are public by design), `WHATSAPP_OWNED_PHONE_E164`,
`WHATSAPP_PHONE_NUMBER_ID`, `MUX_ENVIRONMENT_ID_DEV/PROD`, and every `NEXT_PUBLIC_*` value — those are
shipped to browsers on purpose, so rotating them protects nothing.

## After rotating, verify

```sh
pnpm scan:secrets    # must stay green; the baseline must not grow
pnpm install --lockfile-only && pnpm db:parity   # nothing else should have moved
```

Then record it: `pnpm story:status --ids ENG-FORGE-SECRET-HISTORY-01 --status Complete --reason "<who
rotated what, when>"`. **The story stays open until then on purpose** — a security story marked done
while the values still work is the one kind of bookkeeping that costs real money.

# Credential rotation checklist — the 2026-09-03/04 checkpoint capture

## Severity, stated honestly: THIS IS NOT URGENT, and it may not be needed at all

Written 2026-09-18, then corrected the same night after the captain asked the obvious question: *"why am
I losing 30 minutes of my life over credentials backed up in a git repo that is private behind 2FA?"*
He was right to push back, and the premise of the original version of this file was wrong.

**1. The environment backup NEVER LEFT THIS MACHINE.** `.env.local.before-icloud-username-fix` was
captured by **Cline checkpoint refs** — local shadow refs under `refs/cline/checkpoints/` that snapshot
*untracked* files. Three independent checks agree that nothing was pushed:

- the gitleaks scan of the **remote** (2,103 commits) reports nothing;
- **GitHub secret scanning shows zero alerts** on this repository;
- the blob is gone locally (`git cat-file -e 297c48e2` → *not a valid object*), and no branch or remote
  ref ever held it.

So the exposure is a **local-disk** exposure, not a public one. Private-vs-public and 2FA were never
doing the protecting here; *"it never left the machine"* was. The one question that still matters is
therefore narrow: **has a copy of this laptop's disk ever been reachable by someone else** — an
iCloud/Time Machine backup on a shared volume, a NAS, a repair shop, a resale? If the answer is no,
the 30 minutes buy approximately nothing.

**2. The three findings that ARE in public history are not live credentials.** Anyone can clone these,
and it still does not matter, because of what they contain:

| File (deleted from the tree) | What was actually committed |
| --- | --- |
| `app/api/internal/signature-provision/route.ts` | `TOKEN_SHA256 = '…'` — a **sha256 digest**, compared against `sha256(query param)`. A one-way value: the token itself is not in the repo. |
| `app/api/internal/signature-provision-reset/route.ts` | same shape — a digest, not a token |
| `app/api/internal/jessica-listing-v4/route.ts` | `ONE_TIME_TOKEN = '…'`, a handoff token for one internal page |

And every one of those routes answers **404 in production** today, and none exists in the tree. A
digest cannot be reversed into an authorization, and a one-time token for a page that no longer exists
guards nothing. Rotation here would be ceremony.

## So what is the actual act?

**Nothing tonight.** Then, only if the disk has ever been backed up somewhere shared or the machine has
been out of your hands: rotate the two with real blast radius — the **production database password**
and **`AUTH_SECRET`** — and leave the rest, because the remainder is revocable in seconds if anything
ever does go wrong, which is a cheaper hedge than an evening of console work.

The table below is kept as REFERENCE, not as a to-do list. If you ever want to work through it, it is
30 minutes of console clicks with no urgency attached.

## The captain's operating model (recorded 2026-09-18, so no agent re-litigates it)

**1. Checkpoint refs stay ON, because they ARE the backup.** VS Code / Cline snapshots the workspace
into `refs/cline/checkpoints/`, and on this machine that is a *deliberate* safety net for large
refactors — turning it off to reduce git noise would cut off the restore points exactly when they are
most wanted. The credential class is closed another way, at the source: `.gitignore` says `.env*` with
`!.env.example`, and every spelling of an env file is ignored (verified with `git check-ignore -v` on
`.env`, `.env.local`, `.env.local.before-icloud-username-fix`, `.env.production.local`,
`.env.local.bak` — all matched by rule `.env*`), so a checkpoint can no longer capture one. Purging old
sessions is the captain's periodic choice, NOT a cleanup an agent performs on its own initiative.

**2. iCloud backs this machine up — which is why rotation is low-value here, not high.** The live
`.env.local` sits on the same disk that iCloud copies, so anyone who could reach a backup could reach
today's values too. Rotation therefore only invalidates copies taken *before* the purge; it does not
defend against backup exposure, because the same path exposes the current secrets. Said plainly: if the
disk is the threat, the disk is already the threat and rotating is not the fix. It becomes worth doing
only if the captain judges a *historic* copy got loose somewhere the current one did not.

**3. BoldSign is expensive, necessary, and not an open gateway.** The signature flow was three weeks of
work: a callback to a signature provider, gated by **four secret/webhook variables**. None of those is
in the public findings. What is in public history is the internal *provisioning* routes — two of them
gated by `TOKEN_SHA256` (a one-way **digest**, compared against `sha256(query param)`, so the token is
not in the repo) and one by a `ONE_TIME_TOKEN` that was a mistake. All three routes answer **404 in
production** and none exists in the tree. There is nothing to rotate here, and nothing about BoldSign
is exposed.

**4. CI checks; the captain ships. Batch rhythm: ~30 stories → build → deploy.** Now that the CI deploy
job is deleted, GitHub Actions only runs the gates — free on a public repository, no Vercel builds — and
`pnpm release` from the captain's machine is the whole deploy path: 30 stories, then one build and one
deploy. Flipping CI deploys back on is a decision he may take later; it is not an automation gap for an
agent to close.

## What the tool did and did not earn

gitleaks found something genuinely worth knowing: that a plaintext env backup had been captured at all,
by a mechanism (checkpoint shadow refs) that no human would have suspected, and that the `.gitignore`
rule `.env*.local` could not match a name ending in `-fix`. That is a real find, and the fix — scan in
CI, `.gitignore` → `.env*` with `!.env.example`, purge the refs — is real.

What a scanner cannot do is set the priority. Deferring to it — as the first version of this file did,
and as I did when I put rotation at the top of the captain's list — turns a receipt into an obligation.
A finding is input to a risk decision. The decision is his.

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

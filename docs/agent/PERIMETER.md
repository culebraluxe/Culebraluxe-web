# The perimeter: instruments, what they found, and the traps in running them

**Written 2026-09-18** by Cline, after the operator installed three tools on GPT's recommendation
(squawk, gitleaks, osv-scanner) and the first honest runs of each. This file records what the
instruments are FOR, what they actually found, and the invocations that do not work — because a tool
that runs is not a tool that covers, and this repository has now been bitten by that distinction
twice in one day.

## The instruments

| Job | Instrument | Invocation | In the gate |
| --- | --- | --- | --- |
| Migration safety | squawk 2.65.0 | `pnpm scan:migrations` (changed files; `--all` for the history) | `gates.yml` step |
| Credentials in history | gitleaks 8.30.1 | `pnpm scan:secrets` | `gates.yml` step |
| Dependency advisories | osv-scanner 2.6.0 | `pnpm scan:deps` | `gates.yml` step |
| Deploy boundary | GitHub Actions | `.github/workflows/gates.yml` | itself |

Config: `.gitleaks.toml` (rules + allowlists), `.gitleaksignore` (the known historical exposure),
`scripts/scan-migrations.sh` (the changed-file logic), `package.json` (the three scripts).

## What the first runs found — the numbers, because they are the argument

**Credentials (gitleaks, first run in this repository's history).** A committed backup of the
environment file — `.env.local.before-icloud-username-fix` — sits in 9 commits dated 2026-09-03/04,
carrying 48 variable names, including `DATABASE_URL_PROD`, `AUTH_SECRET`,
`WHATSAPP_ACCESS_TOKEN`, `BOLDSIGN_API_KEY`, `ICLOUD_MAIL_APP_PASSWORD` and `XAI_API_KEY`. The file
is gone from the tree; history is not. **Root cause, fixed the same day:** `.gitignore` said
`.env*.local`, which requires the name to END in `.local`, so the backup was never ignored. It now
says `.env*` with `!.env.example`. The exposure and its rotation are tracked by
`ENG-FORGE-SECRET-HISTORY-01` — the baseline in `.gitleaksignore` is a receipt of what is known, not
permission to keep it.

**Migrations (squawk, all 192 files).** 745 findings: 145 indexes created without `CONCURRENTLY`,
161 missing `lock_timeout`, 161 missing `statement_timeout`, 50 constraints added without
`NOT VALID`, 15 foreign-key additions, 5 column-type changes, 3 dropped columns. The newest 25
migrations still carry 108 of them. The already-applied files are archaeology, so the gate runs on
**changed files only** — a gate that is red on day one is a gate everyone learns to ignore.

**Dependencies (osv-scanner).** 17 packages affected by 45 advisories: 2 critical, 22 high, 19
medium, 2 low — `sharp 0.35.3` (8.9), `qs 6.15.2`, `next 16.3.0`, `postcss`, `brace-expansion`,
`nanoid`. Triaged as advisory-not-exploitable; `ENG-FORGE-DEPENDENCY-AUDIT-01` carries the triage.

## The traps — each one measured, each one quiet

1. **`osv-scanner … -L pnpm-lock.yaml` without `--all-packages` reported 17 packages against a
   lockfile holding 899.** It resolves an unusable subset and exits successfully, so it looks like a
   clean answer. The script pins the flag.
2. **`gitleaks dir` is not the perimeter.** Directory mode walks `.gitignore`d paths too, so it
   reported 209 findings that were mostly the operator's own local `.env.local` and `.next/` build
   output. History (`gitleaks git`) is the perimeter; the working tree is not.
3. **`npm install` cannot run in this repository at all.** `npm install zod` dies with
   `TypeError: Cannot read properties of null (reading matches)` from `Link.matches` in npm's
   arborist: npm cannot build its ideal tree over pnpm's symlinked layout. Use `pnpm add`.
4. **A rule that is too broad is a rule that hides things.** The first custom BoldSign rule matched
   test fixtures and would have baselined 50 findings in live source files as "known exposure". The
   rules now carry entropy floors and per-rule path allowlists, and the baseline fell from 183
   entries to 147 — all of them the real incident.

## The deploy boundary (not yet a fence)

`gates.yml` runs on push and pull request, but **Vercel deploys `main` directly**, so a failing
commit is already serving by the time the check reports. Making the deployment the protected
boundary is `ENG-FORGE-CI-PRODUCTION-FENCE-01`. Until then, a red run is an alert.

The workflow was validated with `actionlint` 1.7.12 (exit 0) and **has never executed**. The two
third-party action invocations (gitleaks and osv-scanner) are the untested part; the story says so
rather than implying otherwise.

## What is deliberately NOT here

A SAST platform (CodeQL) — semgrep already covers pattern and security rules, and a second opinion
nobody reads is cost without coverage. Browser automation — a tool outside its catalog grant is an
authority leak, not a capability. A second linter — eslint is wired; replacing it now is churn.
Repository-wide mutation testing — `ENG-FORGE-FENCE-CAN-FAIL-01` asks a narrower and more useful
question (can this story's own proof fail?), and Stryker is scoped to pure policy modules if it ever
lands.

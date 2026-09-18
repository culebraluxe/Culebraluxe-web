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

**Credentials (gitleaks, first run in this repository).** A plaintext backup of the environment file
— `.env.local.before-icloud-username-fix` — is captured in **9 `refs/cline/checkpoints/*` refs**
dated 2026-09-03/04, carrying 48 variable names including `DATABASE_URL_PROD`, `AUTH_SECRET`,
`WHATSAPP_ACCESS_TOKEN`, `BOLDSIGN_API_KEY`, `ICLOUD_MAIL_APP_PASSWORD` and `XAI_API_KEY`.

**It was never pushed.** The blob (297c48e2) is reachable from exactly those nine checkpoint refs and
from **no branch and no remote ref**; `gh api .../contents/.env.local.before-icloud-username-fix` is
404 and the commit is not on the remote. The mechanism also explains how it "slipped in" with nobody
staging it: **Cline checkpoints snapshot untracked files**, so a backup that no human ever
`git add`ed was captured by a tool — and the old `.gitignore` rule `.env*.local` could not match a
name ending in `-fix`, so nothing stopped it.

**Root cause, fixed the same day:** `.gitignore` now says `.env*` with `!.env.example`. Remaining
risks, not inflated: the secrets sit in local git objects that outlive the working file; `git push
--mirror` would publish every ref including `refs/cline` (`git push --all` would not — it carries
`refs/heads` only); and any full copy of this clone carries them. Rotation is prudent for the
cheap-to-regenerate crown jewels but is not an emergency. Tracked by
`ENG-FORGE-SECRET-HISTORY-01`; the `.gitleaksignore` baseline is a receipt of what is known, not
permission to keep it.

**PURGED, 2026-09-18 (operator's call, at the operator's instruction).** The nine checkpoint refs were
deleted (`git update-ref -d`, 1179 → 1170 refs under `refs/cline`), then `git gc --prune=now` dropped
the object: `git cat-file -e 297c48e2` goes from reachable to **gone**, `git fsck` is clean, `main` and
the tree are untouched. The baseline shrank from **147 entries to 3** — purging is the remediation the
baseline was waiting for, and a baseline that does not shrink when the exposure is removed is a
baseline that has become an allowance. The three remaining entries are `generic-api-key` findings in
deleted internal API routes that still sit in checkpoint refs (kept deliberately: they are not
environment files, and each one costs more restore points). Cost paid, stated plainly: the Cline
restore points tied to those nine refs are gone. What this does NOT do: un-expose anything already
copied off this machine, which is why rotation remains the only remedy that ends the risk.

## CORRECTION, 2026-09-18 — my own wrong alarm, kept because it is instructive

The first version of the section above said the credentials were **in git history** and told the
operator their production keys were published and must be rotated. That was wrong, and it was wrong
in a way worth writing down rather than quietly editing:

1. **"Commits" is not "history".** gitleaks said 9 commits; I read that as repository history and
   never asked which *refs* held them. The answer was nine shadow refs under `refs/cline/checkpoints/`
   — of 1179 such refs — and no branch at all.
2. **A tool's `Link` field is not evidence.** gitleaks emits
   `https://github.com/culebraluxe/Culebraluxe-web/blob/<sha>/...`, which *looks* like a published
   URL. It is the tool constructing a plausible URL from the remote name. I read it as publication.
   The check that actually answers the question is `gh api repos/.../commits/<sha>` — which returns
   422 for these.
3. **The lesson generalises to every instrument in this file.** Each one produces a finding that
   *feels* authoritative and carries a field that looks like proof. Before telling the operator
   something is on fire, ask which ref, which environment, which authority — and make the tool's
   own output show its work. The alarm was cheap to check and expensive to send.

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
5. **A squawk exception must sit IMMEDIATELY above the REPORTED LINE — which inside a multi-line
   statement is the COLUMN, not the statement.** `-- squawk-ignore <rule>` is honoured only as the last
   comment line before that line: put prose in between and the finding stands, and put it above a
   `create table` when the violation is reported on a column inside it and it does nothing at all.
   Both mistakes were measured on 2026-09-18: `192_forge_batch_release_receipt.sql` (prose in between,
   finding stood) and `194_stellar_listing_details.sql` (directive above the statement, while
   `prefer-bigint-over-int` was reported on the `tax_year integer` line).
6. **`CREATE INDEX CONCURRENTLY` cannot run in this repository at all** — and that is a real limitation,
   not a preference. `scripts/apply-migration.mjs` executes an entire migration file as one
   `pool.query(sql)`, and PostgreSQL runs a multi-statement simple query in a **single implicit
   transaction**, inside which `CONCURRENTLY` is refused outright. No migration here uses it, for that
   reason. So the index on a busy table WILL take a write lock, and squawk's safest available pattern is
   unavailable until an applier can run a statement outside a transaction. Where that matters now, the
   exception is recorded in the migration file itself with its reason; the durable fix belongs to
   `ENG-FORGE-MIGRATION-LINT-01`.
7. **A push that reports `exit=0` is not evidence a commit exists.** On 2026-09-18 a multi-line commit
   message was written as a shell heredoc; the terminal ate it, so no commit was ever created — and the
   very next `git push` returned **0** because there was nothing to send ("Everything up-to-date"). Two
   green signals, neither of them about the work. The guard is to verify the object before believing the
   transport: `git log --oneline -1` (is the message the one I wrote?) and
   `git rev-list --left-right --count origin/main...HEAD` (is it ahead?). This is the same family as
   `typecheck | tail` reading a pipe's exit code, and it is why a claim of "pushed" gets the SHA attached.
8. **A cleaner that cannot see landed work never cleans — and reports health while it doesn't.** The
   counter-wipe (`pnpm health`) judged a branch with `git branch --merged main`, which asks whether the
   branch tip is an **ancestor** of main. Work here lands by rebase and cherry-pick, so a branch whose
   every patch is already in main is still not an ancestor of it. Measured 2026-09-18: `--merged` said
   "not merged" about 106 local branches while `git cherry` said "already applied" about their patches;
   168 branches had accumulated (including 59 test fixtures from dogfood and smoke runs), and
   `git branch -d` refuses the same branches for the same wrong reason — so the fix needed a FORCE
   delete with the proof established first. The same wrong test sat in the worktree section, where it
   meant a rebase-landed worktree was never removed. Landedness is now judged by **patch**, in
   `lib/git/branch-hygiene.ts`, with two rules that are not about proof: a branch checked out in a
   worktree is never touched (`cmd-01` is a live second checkout), and a branch a PERSON named
   (`feat/*`, `v0/*`, `demo-lockdown/*`) is reported, never deleted — 29 remote branches were provably
   landed and most of them were someone's.
9. **A fence can prove a setup the engine does not have.** The worker commit path passed no git
   identity, so it used the machine's — and on a machine with no git config that is git's placeholder:
   **228 commits on this repository are authored by `Your Name <you@example.com>`** (all
   2026-08-23..28; real work, attributed to nobody). The fence never noticed because the fence
   configured an identity (`user.email = eng21@test`) in its temp repo, so it asserted a commit's
   contents while the file never asserted the **author**. Fixed by stamping `GIT_AUTHOR_*` and
   `GIT_COMMITTER_*` on every commit the path makes (`lib/worker-workspace/commit.ts`, also used by the
   learn loop's commit), and fenced the other way round: the new test builds a repo whose config IS the
   bug and asserts the commit comes out as Forge Smith.

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

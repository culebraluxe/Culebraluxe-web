# DEV Database Setup

How this workspace points at the **disposable DEV Neon database** and how to
apply schema changes there. The DEV database is a Neon branch — safe to
recreate, reset, and mutate freely. **Never touch the `production` branch.**

## Current setup (as of 2026-08-21)

- Neon project: `snowy-salad-48970537` (org `org-bitter-darkness-09990307`).
- DEV branch: **`dev`** (id `br-solitary-star-axgusezm`, endpoint
  `ep-muddy-lab-axtgckj9`), created from `production` as parent.
- The `.neon` file and `NEON_BRANCH` are set to `dev` locally.
- `.env.local` (gitignored) points every DEV access variable at the dev
  branch:
  - `DATABASE_URL_DEV` → pooled endpoint
    (`ep-muddy-lab-axtgckj9-pooler.c-4.us-east-2.aws.neon.tech`)
  - `DATABASE_URL_UNPOOLED` → direct endpoint
    (`ep-muddy-lab-axtgckj9.c-4.us-east-2.aws.neon.tech`)
  - `DATABASE_URL` (both lines) → pooled endpoint, so any code that reads
    `DATABASE_URL` directly (e.g. `workflow_engine/lib/workflow/db.ts`) never
    reaches production
  - `DATABASE_URL_PROD` → **unchanged, still production** — never edit this

## How DEV access is resolved

`db/client.ts` selects the URL by `APP_ENV`:

```
APP_ENV === "production"  → process.env.DATABASE_URL_PROD
otherwise                 → process.env.DATABASE_URL_DEV
```

`.env.local` sets `APP_ENV="development"`, so the app and all `scripts/*` use
`DATABASE_URL_DEV` (the dev branch). `lib/neon-interactive.ts` (interactive
transactions) and `db/tx.ts` also resolve through `db/client.ts`.

## Applying migrations to DEV

Scripts run outside Next must load `.env.local` explicitly — the repo pattern
is:

```
node --env-file=.env.local node_modules/tsx/dist/cli.mjs <script>.ts
```

To apply a migration, execute the statements from the file inside one
transaction against the dev URL (see the pattern used for
`db/migrations/021_storyboard_story.sql`). Two gotchas:

1. **Semicolons inside string literals.** Titles/notes legitimately contain
   `;`. Never split a migration file on `;` blindly — use a quote-aware
   splitter that only splits outside single-quoted strings.
2. **The Neon driver parameterizes interpolated strings.** A query like
   `` sql`select ${cols} from ...` `` becomes `select $1` (a `?column?` row).
   Column lists must be written **literally** in every query.

## Recreating the disposable DEV branch

Non-default Neon branches auto-expire per `.neon` branch policy (TTL 7 days).
When the dev branch is gone or stale:

```
cd <workspace>
neon branches create --name dev --parent production
neon branches list            # confirm "ready"
neon connection-string dev    # direct (unpooled) URL; use -pooler host for pooled
```

Then update `.env.local`: `DATABASE_URL_DEV` / `DATABASE_URL` (pooled host),
`DATABASE_URL_UNPOOLED` (direct host), `NEON_BRANCH=dev`, and re-apply
`db/migrations/021_storyboard_story.sql`.

## Story Board on DEV

- `db/migrations/021_storyboard_story.sql` (table),
  `db/migrations/022_storyboard_authoritative_seed.sql` (completion/rollup
  columns + the 74-story 8/21 master board), and
  `db/migrations/023_storyboard_execution_history.sql` (story dates + the
  eight-value status CHECK + `storyboard_story_run` execution history) are
  **applied to the DEV branch** (2026-08-21). They are **not** applied to
  production.
- Story IDs are the human-assigned master IDs (CRM-*, OPS-*, PORTAL-*, PX-*,
  PLAT-*, ENG-*, POLISH-*, AUTH-*, DOC-*). Workstream values are the canonical
  short codes: PUBLIC, CRM, PORTAL, TXN, ADMIN, AUTH, CONTENT, HARDEN.
- Statuses are exactly: Planned, In Progress, Complete, Partial, Blocked,
  Failed, Deferred, Hold. Completion math uses the stored `completion` (0..100);
  Complete forces 100. Net-Net = Σ (workstream completion × weight), where
  workstream completion = AVG(stored completion) over rollup stories.
- Execution runs: `startStoryRun` sets In Progress + preserves the first
  `actual_start_at`; `finishStoryRun` records the run (result, completion,
  notes, commit hash, tests summary) and updates the parent story without
  touching human notes. Run history is surfaced per story on the board.
- The repository tests in `workflow_app/tests/storyboard.test.ts`,
  `workflow_app/tests/storyboard-rollup.test.ts`, and
  `workflow_app/tests/storyboard-runs.test.ts` use in-memory fakes / pure
  models and never require a database.

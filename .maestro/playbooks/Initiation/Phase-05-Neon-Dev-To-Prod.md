# Phase 05: Neon DEV → PROD Migration

This phase runs when the work card is a `db` card, or when an earlier phase left it at `status: needs-db`. It writes or verifies one migration file, applies it to DEV (`ep-muddy-lab-axtgckj9`), and proves it there. It then takes a Neon snapshot or backup branch of PROD (`ep-flat-art-ax92tn7a`) so a rollback always exists, applies the migration to PROD, and confirms with `pnpm db:parity` that the two environments agree. The procedure follows `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` and `docs/agent/DEV-OPS-RELEASE.md`, which override this document wherever they are more specific. Claude has full authority on both environments, but no PROD statement runs without a rollback point taken first. For any other card, every task records `N/A` and ticks.

## Tasks

- [ ] Gate on the card. Read `.maestro/playbooks/Initiation/Working/CARD.md`. Continue only if `kind: db` with `status: open`, or `status: needs-db`. Otherwise append `Phase 05: skipped` to the card's `## Log` and tick every remaining task in this phase with `N/A`. Also check `Working/holds.md`: if a PROD-migration hold names another owner and covers this migration, apply the change to DEV only. In that case, record `PROD held by <owner>` and tick the PROD tasks with `N/A (held)`.

- [ ] Read the contract before writing SQL. Read `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` in full, including the sections on `db-tool apply`, the `schema_migration` record and parity's blind spots (functions and views). Run `pnpm db:migrations` and `pnpm db:parity` and save both outputs to `Working/logs/db-before-<date>.log`. If parity already shows drift unrelated to this card, record it in the card and do not "fix" it in this migration.

- [ ] Write or validate the migration file at `db/migrations/<NNN>_<snake_name>.sql`, using the next free number:
  - Match the style of the last five migrations (header comment, `IF NOT EXISTS` and idempotence where the house style uses it, explicit constraint names).
  - Make it safe to run once on both environments.
  - Prefer additive changes. For any destructive statement (drop, type narrowing, data delete), add a comment that names the backup branch it relies on.
  - Run `pnpm scan:migrations`.

  Never use a throwaway probe file to test access, because `apply` records history.

- [ ] Apply to DEV and prove it. Run `pnpm db:migrate db/migrations/<file>.sql dev`. Then:
  - Run `pnpm db:migrations` to confirm the DEV row was recorded.
  - Run the tests that read the changed schema (`pnpm test:app:db`, plus the card's acceptance command).
  - Spot-check the shape with the Neon MCP `describe_table_schema` tool or with SQL against DEV.
  - If anything fails, fix the migration with a new forward migration (never by editing the recorded file), and re-apply on DEV.

  Save the output to `Working/logs/db-dev-<date>.log`.

- [ ] Create the PROD rollback point before any PROD write. Use the Neon MCP `create_snapshot` tool, or `create_branch` with a parent of the PROD branch, named `pre-<NNN>-<YYYYMMDD-HHMM>`. Fall back to the Neon CLI or the method `DEV-OPS-DATABASE-PLAYBOOK.md` names. Confirm the snapshot or branch exists with `list_snapshots` or `list_branches`, and record its id in the card under `## Rollback`. If no rollback point can be created, stop. Set the card to `status: blocked-no-backup` and tick the remaining PROD tasks with `N/A (no backup)`. A PROD write without a rollback point is not allowed. <!-- MAESTRO:MODEL tier="high" effort="medium" reason="This step is the only safety net for production data. It must verify the snapshot really exists against the right branch, which takes care rather than deep design." -->

- [ ] Apply to PROD and verify: <!-- MAESTRO:MODEL tier="high" effort="high" reason="A production schema change on the database the Forge engine and live site both run against. A mistake corrupts live data or halts engine runs, so this needs the strongest model checking every result." -->
  - Run `pnpm db:migrate db/migrations/<file>.sql prod`.
  - Run `pnpm db:migrations` to confirm the PROD row and that nothing is now one-sided.
  - Run `pnpm db:parity`. All five axes must show zero drift for this change. Separately verify any functions or views the migration touched with `pg_get_functiondef` or `pg_get_viewdef` on both sides, because parity does not read them.
  - Run `pnpm forge:doctor` to confirm the engine is healthy against PROD.

  If verification fails, record the failure and the snapshot id. Restore from the rollback point only if PROD is broken (the engine or the site fails), not merely drifted, and record exactly what was restored.

- [ ] Land the code and close the card:
  - Commit the migration file (plus any held code slices from Phase 03 or 04) as `feat(db): <migration purpose> (NNN)`, or `fix(db): …`.
  - Run `git fetch origin main && git rebase origin/main && git push origin HEAD:main`, then `git push --force-with-lease origin HEAD:lane/claude`.
  - Record the shas and the snapshot id in the card's `## Log`.
  - Strike the inbox line and set the card to `status: done`.

- [ ] Verify Phase 05:
  - `pnpm db:migrations` shows the migration recorded on both DEV and PROD, unless PROD is held.
  - `pnpm db:parity` reports no drift.
  - `git log origin/main` contains the migration commit.
  - The snapshot or branch id is in the card.

## Manual Follow-Up (not executed by Auto Run)

- Delete the `pre-<NNN>-…` Neon backup branch or snapshot once you are satisfied the change is stable, to stop paying for storage.
- Deploy the app to production with `pnpm deploy:prod` when you want the code half live; it is a deliberate release step.

# Phase 01: Sync & Health Check

This phase brings `lane/claude` level with `origin/main`, runs every cheap gate the repo owns, and turns the results into one dated health report plus a ready-to-use `INBOX.md`. It is the start of every Claude-lane session: the result is a written picture of what is red, what is stuck in the Forge engine, and what is armed, so the debug, feature and database phases that follow start from facts instead of guesses. It needs no input from anyone and changes no product code.

## Tasks

<!-- MAESTRO:MODEL tier="low" effort="low" reason="Running known commands and recording their output is mechanical. The single judgment step, classifying failures, gets its own marker below." -->

- [ ] Orient before touching anything. Read `AGENTS.md`, `docs/agent/ORIENTATION.md`, `docs/agent/MEMORY.md`, `docs/agent/CURRENT.md`, the newest `docs/agent/QUEUE-*.md`, and the newest two `docs/agent/HANDOFF-*.md` files (sort by the date in the filename). Note every HOLD a handoff lists (for example the `.gitleaksignore` hold and the PROD-migration hold) in `.maestro/playbooks/Initiation/Working/holds.md`. Write it as a markdown table with columns `hold | owner | source file | what an agent must do`. Add YAML front matter with `type: reference`, `title: Active Holds`, `created: <today>`, and `tags: [holds, claude-lane]`. Create the `Working/` folder if it is missing. Later phases read this file and must obey it.

- [ ] Sync the lane with trunk without losing work:
  - Run `git status --porcelain`. If the tree is dirty, commit the changes as `chore(lane): wip before sync` (never stage `.env.local` or any secret file) rather than stashing. The stash stack is shared across worktrees.
  - Run `git fetch origin main && git rebase origin/main`. If the rebase conflicts, resolve conflicts in files this lane changed. If a conflict is outside this lane's own work, run `git rebase --abort` and record the conflict in the health report as RED instead of guessing.
  - Record `git log --oneline -1`, `git rev-parse origin/main`, and the ahead/behind counts from `git rev-list --left-right --count origin/main...HEAD`.

- [ ] Run the harness gates and save the raw output under `.maestro/playbooks/Initiation/Working/logs/` (one file per command, with stdout and stderr captured, and the exit code appended on the last line):
  - `pnpm forge:sync-agents --check`
  - `pnpm forge:packet-lint`
  - `pnpm test:harness`
  - `pnpm forge:batch:status`
  - `pnpm db:migrations` (read-only: it reports what is recorded on DEV and PROD and what is one-sided)

- [ ] Run the compile and test tiers and save the output the same way. Each command is long-running, so use generous timeouts:
  - T0: `cargo check --workspace --all-targets`
  - T1: `pnpm slice:check`. If the script does not exist, fall back to `pnpm test` (this runs `cargo test -p workflow -p forge -p test-harness`).
  - `pnpm scan:secrets` and `pnpm scan:migrations`.
  - Do not fix anything in this task; only capture what you find.

- [ ] Classify every failure from the logs by owner and write `.maestro/playbooks/Initiation/Working/health-report-<YYYY-MM-DD>.md`: <!-- MAESTRO:MODEL tier="medium" effort="medium" reason="Attributing a failure to engine, role-service hook, product or environment requires reading the failing code. A wrong owner sends the debug phase down the wrong path." -->
  - Front matter: `type: report`, `title: Claude Lane Health <date>`, `created`, `tags: [health, claude-lane]`, `related: ['[[holds]]', '[[INBOX]]']`.
  - Section `## Sync` holds the HEAD sha, the origin/main sha, ahead/behind counts, and the rebase outcome.
  - Section `## Gates` holds a table with columns `gate | command | result GREEN/RED/SKIPPED | log file`.
  - Section `## Failures` has one row per distinct failure with the columns `test or check | file:line | owner (engine / role-service hook / product / harness / environment) | is it covered by a HOLD? | one-line suspected cause`. Following the owner's design, role behaviour belongs in each role service's hooks on `AbstractForgeService` and never in engine or harness code. Attribute ownership accordingly.
  - Section `## Forge engine state` summarizes `pnpm forge:batch:status`: running, stuck, armed and failed stories.
  - Section `## Suggested next asks` lists up to five one-line candidate INBOX items derived from the RED rows. Each is prefixed with `[debug]`, `[feature]` or `[db]`.

- [ ] Create the inbox the later phases consume, at `.maestro/playbooks/Initiation/INBOX.md`. Do not overwrite it if it already exists; append instead.
  - Front matter: `type: note`, `title: Claude Lane Inbox`, `tags: [inbox, claude-lane]`.
  - A short header explaining the format: one line per ask, as `- [debug|feature|db] <one-line ask>`. The topmost un-struck line is the next item to work. A finished line is struck through (`~~...~~`) with the landing commit sha appended.
  - Under `## Suggested by health check <date>`, copy the `Suggested next asks` from the report as **commented-out** lines (`<!-- - [debug] ... -->`), so the user decides what is live.
  - If the inbox has no live line, add one live default: `- [debug] Fix the highest-priority RED row from the latest health report that is not covered by a HOLD`. If the report has no unheld RED row, add `- [feature] Pick the next Ready story from docs/agent/QUEUE-*.md and build it` instead.

- [ ] Commit and land the sync. If the rebase produced commits or the WIP commit exists, push with `git fetch origin main && git rebase origin/main && git push origin HEAD:main`, and then `git push --force-with-lease origin HEAD:lane/claude`. If a pre-push hook refuses, do not retry the same command. Record the refusal and its message in the health report under `## Push` and continue. The `.maestro/playbooks/Initiation/` working files are lane-local scratch; do not commit them.

- [ ] Verify Phase 01 is complete:
  - `Working/health-report-<date>.md`, `Working/holds.md` and `INBOX.md` exist.
  - Every gate listed above has a row in the Gates table.
  - `git status --porcelain` shows no tracked changes outside `.maestro/`.
  - Print the Gates table and the Failures table to the run output as the phase summary.

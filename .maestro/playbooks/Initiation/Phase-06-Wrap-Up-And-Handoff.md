# Phase 06: Wrap-Up & Handoff

This phase closes the session the way `AGENTS.md` requires: nothing is left unpushed, and a handoff in the shape of `docs/agent/HANDOFF-TEMPLATE.md` is on `main`. That handoff states what is true, what is held, what landed and with which receipts, what is still open in order, and what was not verified. The next agent, or the next run of this playbook, can then start cold from facts. The phase also resets the work card so the playbook can loop on the next inbox line.

## Tasks

<!-- MAESTRO:MODEL tier="low" effort="medium" reason="Assembling a handoff from logs and the card is mostly transcription. It still needs care to report only gates that actually ran." -->

- [ ] Make sure nothing is stranded:
  - Run `git status --porcelain` and `git log origin/main..HEAD --oneline`.
  - Commit any tracked change this run made but did not commit, in house style. Never commit `.env.local`, secrets, or `.maestro/playbooks/Initiation/Working/` scratch files.
  - Land with `git fetch origin main && git rebase origin/main && git push origin HEAD:main`, then `git push --force-with-lease origin HEAD:lane/claude`.
  - If a push is refused, do not retry blindly. Run `pnpm wip:now` to snapshot the work locally and record the refusal for the handoff.

- [ ] Run a final health pass on the landed HEAD:
  - `pnpm forge:packet-lint`, `pnpm test:harness`, `pnpm forge:batch:status`, and `pnpm db:migrations`.
  - The card's acceptance command, if the card was worked.

  Compare the results with the Phase 01 `Working/health-report-<date>.md`, then append a `## Delta` section to that report. Its table has the columns `gate | before | after`, followed by a list of newly GREEN and newly RED items. A newly RED item that this run caused must be fixed and landed before you continue. If a fix is not possible within this task, it goes into the handoff as OPEN item #1.

- [ ] Write the handoff. Copy `docs/agent/HANDOFF-TEMPLATE.md` to `docs/agent/HANDOFF-claude-lane-<topic>-<YYYY-MM-DD>.md`, where `<topic>` is a 2–3 word slug of the card. Fill every section and delete the template italics:
  - **STATUS**: facts, each with a `path:line` or a command.
  - **HOLDS**: from `Working/holds.md`, plus any new ones, each with its owner.
  - **WHERE TO LOOK**.
  - **DONE**: one row per landed sha on `origin/main`, with the exact gate that ran and its result. Include the Neon snapshot or branch id if Phase 05 ran.
  - **OPEN**: in priority order, including blocked or held inbox lines.
  - **NOT VERIFIED**: be honest here; any gate skipped or not run goes in this section, not under DONE.

  Keep it short: one row per fact, one line per action.

- [ ] Update the living docs only where this run changed the truth:
  - If a fact in `docs/agent/CURRENT.md` is now false because of this run, correct that line and point it at the new handoff.
  - If an item in the newest `docs/agent/QUEUE-*.md` was completed, mark it done with the sha.
  - Do not rewrite sections unrelated to this run.
  - Run `pnpm forge:packet-lint` and `pnpm forge:sync-agents --check`. If the check reports drift, run `pnpm forge:sync-agents` and include the result.

- [ ] Commit and land the documentation as `docs(agent): handoff <topic> <date>` with the same push sequence as above, then confirm with `git log origin/main --oneline -3`.

- [ ] Reset for the next loop:
  - Rename `Working/CARD.md` to `Working/CARD-<date>-<topic>.md`, so the next run of Phase 02 selects a fresh inbox line.
  - In `INBOX.md`, confirm the worked line is struck through with its sha.
  - Append a `## Last run <date>` footer to `INBOX.md` that links `[[HANDOFF-claude-lane-<topic>-<date>]]` and `[[health-report-<date>]]`.
  - Print the handoff's DONE and OPEN tables to the run output as the session summary.

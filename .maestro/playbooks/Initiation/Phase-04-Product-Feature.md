# Phase 04: Product Feature

This phase runs only when the work card is a `feature` card. It makes sure the feature has a story packet in `docs/agent/packets/` that the repo's own lint accepts. It reads the ranked manifest, then builds the feature in thin, tested, landable slices across the Rust tiers (`db/`, `middle/`, `web/`, including the Yew UI in `web/ui`). Each slice lands on `main` as it goes, so a half-finished feature never sits on the lane branch. For any other card kind, every task records `N/A` and ticks.

## Tasks

- [ ] Gate on card kind. Read `.maestro/playbooks/Initiation/Working/CARD.md`. If `kind` is not `feature` or `status` is not `open`, append `Phase 04: skipped (kind=<kind>)` to the card's `## Log` and tick every remaining task in this phase with `N/A`. Otherwise append `Phase 04: started`.

- [ ] Secure the packet. If the card names a story id that already has `docs/agent/packets/<STORY-ID>.md`, read it in full. Otherwise write a new packet:
  - Copy the structure of the three most recently modified packets in `docs/agent/packets/` (heading order, acceptance section, file list).
  - Choose an id that follows the existing prefix conventions.
  - State the acceptance criteria as commands or tests.
  - Run `pnpm forge:packet-lint` until it passes. If `pnpm forge:sync-agents --check` reports drift caused by the new packet, run `pnpm forge:sync-agents`.
  - Record the story id in the card.

- [ ] Design the slices before writing code: <!-- MAESTRO:MODEL tier="high" effort="high" reason="Slicing a feature across db, middle and web tiers sets the interfaces every later slice depends on. A bad cut forces rework or leaves an unlandable half-feature." -->
  - Run `pnpm forge:manifest <STORY-ID>` and read the top-ranked files it returns.
  - Use `ripwire . --exemplar` or `rg` to find the existing building blocks and the best existing example of the same kind of feature, and reuse them rather than inventing parallel ones.
  - Write a `## Slices` section in the card: 2–5 ordered slices. Give each one its files, its test, and its acceptance command, so that each slice compiles, passes and is landable on its own.
  - Mark any slice that needs a migration. That sets `needs_db: true`, and the migration is applied in Phase 05 before any code that reads the new schema lands.

- [ ] Build the slices in order. For each slice:
  - Write its test first, in the owning crate's existing test layout.
  - Implement the slice. Follow the surrounding code's naming and idiom, route failures through the capture framework, never special-case a single listing, and keep role behaviour in role-service hooks.
  - Run `cargo check --workspace --all-targets`. For `web/ui` changes, also run `pnpm ui:check` (or `pnpm ui:build`).
  - Commit with a conventional message such as `feat(<area>): <slice outcome>`.

  Do not push yet if the slice depends on an unapplied migration.

- [ ] Run the gates for the whole feature:
  - The packet's acceptance commands.
  - `pnpm slice:check`, or `cargo test -p <touched crates>` if that script is missing.
  - `cargo clippy -p <touched crates> -- -D warnings`.
  - `pnpm forge:packet-lint`.

  If UI changed, run `pnpm ui:build` and also exercise the affected route through the app server: start it with `pnpm dev`, fetch the page with `curl`, and assert that the expected markup or JSON is present. Fix any regressions you introduced. Pre-existing failures from the Phase 01 report are noted, not fixed here.

- [ ] Land on `main`. If `needs_db: true` and the migration is not yet on PROD, land only the slices that do not read the new schema. Then set the card to `status: needs-db` and list the held commits in the card. Otherwise, run `git fetch origin main && git rebase origin/main && git push origin HEAD:main`, then `git push --force-with-lease origin HEAD:lane/claude`. If the push is refused, read the refusal: fix it if it is your change, and record it otherwise; never retry the same command blindly. Record every landed sha in the card's `## Log`.

- [ ] Verify Phase 04:
  - `git log origin/main` shows the slice commits.
  - Each acceptance command is GREEN on the landed HEAD.
  - The packet passes lint.
  - Strike the inbox line and append the shas.
  - Set the card to `status: done`, or to `needs-db` if a migration is still owed.

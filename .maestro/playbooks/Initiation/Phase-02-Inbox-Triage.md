# Phase 02: Inbox Triage

This phase turns one line from `INBOX.md` into a concrete, scoped work card that the next phases can run without asking anyone anything. It picks the top live ask, classifies it as `debug`, `feature` or `db`, gathers the exact files and evidence it touches, and writes a card that names which of Phases 03–05 does the work. Phases that don't match the card skip cleanly. One inbox line per playbook run keeps commits small and landable.

## Tasks

- [ ] Select the work item. Read `.maestro/playbooks/Initiation/INBOX.md` and take the topmost line that is live: not commented out and not struck through. If there is none, apply Phase 01's default-ask rule using the newest `Working/health-report-*.md`, then append that ask to the inbox. Write the selected line verbatim to the top of a new file, `.maestro/playbooks/Initiation/Working/CARD.md`. The file must have front matter with `type: analysis`, `title: Work Card <date>`, `tags: [card, claude-lane]`, and `related: ['[[INBOX]]', '[[health-report-<date>]]', '[[holds]]']`. If a `CARD.md` from an earlier run exists with `status: open`, keep that card instead of selecting a new item, and continue it.

- [ ] Classify the ask and record a `kind:` field in the card front matter as exactly one of `debug`, `feature`, or `db`. Use the ask's bracket tag if present. Otherwise:
  - `debug` covers something broken, red, stuck or wrong.
  - `feature` covers new behaviour or a packet or story to build.
  - `db` covers a schema or data change, a migration, or DEV/PROD parity.

  An ask that needs a migration *and* code is `feature` with `needs_db: true`, and Phase 05 runs after Phase 04. Also set `status: open`, plus `needs_db: true|false`.

- [ ] Check the ask against `Working/holds.md`. If it would act on a HOLD owned by someone else, set `status: blocked-by-hold` on the card and name the hold. Then strike the inbox line with the note `blocked: <hold>`, select the next live line, and redo the two tasks above for it. If every live line is held, set `kind: none`. Phases 03–05 then skip, and Phase 06 reports why.

- [ ] Scope the work using the codebase's own maps rather than broad file reads: <!-- MAESTRO:MODEL tier="high" effort="medium" reason="Scoping decides which files, owner and story the whole run works on. A wrong scope wastes the expensive phases that follow, but the work is reading, not deep design." -->
  - If the ask names or implies a story id, run `pnpm forge:manifest <STORY-ID>` and read `docs/agent/packets/<STORY-ID>.md`.
  - For a symptom (a failing test or an error string), find the responsible symbol with the `ripwire` CLI (`ripwire . --for "<symptom>"`) or `rg`. Then read only the top two or three ranked files.
  - Check `docs/agent/MAP-engine.md` and `docs/agent/MAP-services.md` for the owning area. Role behaviour belongs in that role service's hooks on `AbstractForgeService`, never in engine or harness code.
  - Record in the card a `## Scope` section with the owner area, the files to touch (path plus the reason for each), the tests that cover them, and any packet or story id.

- [ ] Write the plan into the card. The `## Plan` section contains 3–8 numbered steps, the acceptance check (the exact command that turns GREEN when done), and the gate tier owed (T0/T1 from `AGENTS.md`). For a `db` card also record the target migration filename under `db/migrations/` (next free number after the highest existing one) and whether data in PROD changes. Note any house rule that constrains the fix: the capture framework for every failure, no listing special-cased, no secrets committed.

- [ ] Verify the card is runnable. `CARD.md` must parse as front matter with `kind`, `status: open` and `needs_db`, and its Scope, Plan and acceptance command must be filled in. Every file named in Scope must exist, or be explicitly marked `new`. Print the card to the run output.

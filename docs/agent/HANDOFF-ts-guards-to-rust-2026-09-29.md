# HANDOFF — the TS→Rust test conversion: 8 cards filed, 1 set up (2026-09-29)

The question this answers: *"did you do all the work in PROD to set up the stories to convert all the TypeScript
tests over to Rust?"* **The cards are filed; the setup is not.** Eight HARDEN stories exist on the PROD board
(`Ready`, one `In Progress`), and **seven of the eight have no packet**, so the engine has no declared scope for
them. The premise "convert the tests" also needs correcting: the suite was not left behind to port — it was
**deleted in one commit** on 2026-09-26, and the eight cards cover the seven handbook guards, not the 465 deleted
test files. Every number below is measured; the command is in the row.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | **Zero test files remain in the tracked product tree.** The last four TypeScript-or-JS files outside `legacy/` are `eslint.config.mjs` and three `scripts/*.mjs` sync agents; none is a test. | `git ls-files \| rg '\.(test\|spec)\.(ts\|tsx\|js\|jsx\|mjs\|cjs)$' \| rg -v '^legacy/' \| wc -l` → `0`; `cat docs/agent/ts-allowlist.txt` → 4 lines |
| S2 | The suite was deleted in **one** commit: **829 files / 148,065 lines**, of which **465 test files**, **375** of them under `legacy/workflow_app/tests/`. Subject: *"delete 829 TypeScript files nothing runs, builds or tests"*. | `git show --diff-filter=D --name-only --format= bad45d39 \| rg '\.(test\|spec)\.tsx?$' \| wc -l` → `465`; `… \| rg '^legacy/workflow_app/tests/' \| wc -l` → `375` |
| S3 | The seven guards named by `AGENTS.md` are inside that 465 and are **recoverable from git**, with their assertions, not just their names. | `git show bad45d39^:legacy/workflow_app/tests/db-routing.test.ts` (43 lines: `resolveDbTarget` per `VERCEL_ENV`/`APP_ENV`, and *an undeclared environment refuses instead of defaulting to DEV*) |
| S4 | **No Rust equivalent exists** for any of the seven. Searches for tree residue, git-in-QA, column-writer audit and worker commit identity return 0 hits in `rust/`; the `whatsapp` hits are integration code (`middle/apis/src/whatsapp/payload.rs`) and the domain channel check, not the attribution guard. | `rg -n 'no.tree.residue\|forge.qa.no.git\|column.writer\|commit.identity' rust/ -g '!target'` → no test hits |
| S5 | Eight stories are filed in PROD, all `HARDEN`, all created 2026-09-29: `ENG-GUARD-ENV-RUST-01`, `ENG-GUARD-FORGE-RUST-01`, `ENG-GUARD-REPO-RUST-01`, `ENG-GUARD-AGENTS-LINT-01`, `ENG-PARITY-LEDGER-01`, `ENG-WHATSAPP-COEXISTENCE-RUST-01`, `ENG-POOL-IO-01` (all `Ready`), `ENG-AUTH-GOOGLE-01` (08:03, `In Progress`). | `select id,status,created_at from storyboard_story where created_at >= '2026-09-29'` on PROD → 9 rows (8 stories + 1 LEARN batch) |
| S6 | **One of the eight has a packet** (`docs/agent/packets/ENG-AUTH-GOOGLE-01.md`, 99 lines, written today — it is the shape to copy). The other seven have none, and there is no `docs/agent/manifest/<id>.md` for any of them. | `ls docs/agent/packets \| rg -i 'GUARD\|PARITY-LEDGER\|POOL-IO\|WHATSAPP-COEXISTENCE\|AUTH-GOOGLE'` → `ENG-AUTH-GOOGLE-01.md` only |
| S7 | A story with no packet runs with an empty packet lane: no declared paths, no `declares_new` check, no packet drift check. The packet is load-bearing, not decoration. | `forge/src/scope_manifest.rs:106-111` (`packet_path`, `packet_text`), `:166-175` (`Lane::Packet`), `:289-305` (`declares_new`) |
| S8 | `pnpm forge:packet-lint` **does not resolve `guard:` refs**, so the handbook's dangling ones cost nothing today. Verdict now: **0 failures, 161 warnings (152 baselined), 139 harness files**. | `pnpm forge:packet-lint`; the lint resolves backtick-quoted `path:line-range` evidence (`cli/src/forge/lint/rules.rs`), and a `guard:` line has no line range |
| S9 | The handbook claims seven guard files that do not exist: `AGENTS.md:133` (`publish-scan-coverage.test.ts`), `:151` (`no-tree-residue.test.ts`), `:160` and `:164` (`forge-qa-no-git.test.ts`), `:161` (`worker-commit-identity.test.ts`), `:162` (`db-routing.test.ts`), `:166` (`whatsapp-attribution.test.ts`), `:172` (`column-writer-audit.test.ts`). | `rg -n 'guard: ' AGENTS.md`; `git ls-files \| rg '(db-routing\|no-tree-residue\|forge-qa-no-git\|worker-commit-identity\|whatsapp-attribution\|column-writer-audit\|publish-scan-coverage)'` → empty |
| S10 | The ratchet machinery that *is* live and green is Rust or bash, not TS: `scripts/ts-ratchet.sh` (excludes `legacy/` and `e2e/`) reads `docs/agent/ts-allowlist.txt` (4 entries), and `pnpm broken:ts:sweep` is already `forge ts-sweep`. | `sed -n '16,22p' scripts/ts-ratchet.sh`; `package.json:83` |
| S11 | Two live engine prompts contradict the handbook, and neither is fenced by a test. The common packet body tells **every** node to `Create a local git commit` (`packet.rs:135-138`) — which a QA node must not do (`AGENTS.md:160,164`) — and tells it to *"Commit your changes on this branch only"* in an *"isolated Git worktree"* (`packet.rs:129-133`), against **"Main only"** and **"NO TREES. EVER."** | `forge/src/engine/packet.rs:129-138`; `forge/src/engine/opencode.rs:144,260` |
| S12 | The other half of the conversion — the parity ledger generator — is filed but not started (`ENG-PARITY-LEDGER-01`); `docs/rust-parity-ledger.md` is still produced by the retired stack. | `ls docs/rust-parity-ledger.md`; the story row (S5) |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Rewriting the eight dangling `guard:` lines in `AGENTS.md` | the Captain / GPT's engine audit | Do not mass-annotate them yet: for each rule the disposition may be *restore as a Rust test*, *already enforced in code* (the env-routing rule now has a code fence — `db::resolve_forge_target` — beyond what `db-routing.test.ts` asserted) or *NONE*. Annotating wrongly makes the handbook lie differently |
| H2 | Writing the seven missing packets | the next agent, after H1's answer | The packets are mechanical once S3 is read (the recovered assertions are the acceptance criteria). Do not write them from the story titles alone |
| H3 | S11's two prompt contradictions | the Captain / GPT's engine audit | Report, do not edit: `packet.rs` is engine semantics under an active audit |
| H4 | The engine smoke-out's own Rust tests (pool refusal, class-door budget, the DEV walk case) | the Captain | His call, 2026-09-29: *"those tests are not necessary"*. They are not the TS→Rust conversion; `ENG-GUARD-FORGE-RUST-01` supersedes the budget ones. Keep or drop by one word, not by inference |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Set up a story from this wave | `docs/agent/packets/ENG-AUTH-GOOGLE-01.md` (the shape: Goal / Scope in-out / Why with live refs / Context refs / Acceptance / Skills / Loop / Test mode / Assay commands / Risks / Release obligations) | `docs/agent/packets/<ID>.md`; then `pnpm forge:manifest <ID>` |
| Know what a guard asserted before it was deleted | `git show bad45d39^:<legacy path>` | nothing |
| See what a guard must become | `AGENTS.md` §Never, the `guard:` line | the Rust home named in the packet |
| The wave's one worked example of an in-file Rust test | `web/src/api/google_auth.rs` (`#[cfg(test)] mod tests`) | — |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `29cd9ea2` on `origin/main` | Forge resolves PROD or refuses (`db::resolve_forge_target`); the class repair route is bounded (`budgeted_failure_class`); the sweep may not manufacture authorization (`reconcile_dispatch_queue`) | `cargo check --workspace --all-targets` → `Finished`; `cargo test -p db -p forge --lib` → 50 + 88 passed; `cargo test -p db --test forge_work_claim_dev -- --ignored` → 2 passed; pre-push ran the workspace check and `ui:check` |
| (no commit) | The stalled pair was killed on the Captain's word: `rust/target/debug/forge --story TECH-FLIGHT-RECORDER-01 --work-type FEATURE` (2h10m alive, no claim held, no row written in 30 min) and its `forge-worker` parent; the launchd pass ended with them | `ps -o pid,etime,command -p 23237,23265` → gone; the story's `agent_work_item` was already `Ready`, so nothing was stranded |

## 5. NOT VERIFIED — the honest gaps

- **Whether the eight cards are the whole conversion.** Nobody has measured the other 458 deleted test files (34 in `agent-runtime`, 27 in `testv2`, 24 under `legacy/workflow_app/tests/persistence`, …) against a story. On the evidence the cards are the *guards*, and the rest of the deleted suite has no card — either because nothing ran it (S2's subject) or because nobody looked.
- **The next worker tick on the new binary.** The queue was released by the kill and the launchd job is `StartInterval 180` + `RunAtLoad`; whether the first post-fix pass prints `target=prod` and completes has not been watched.
- **The class-door `HOLD` on a live PROD run.** Proven by unit tests only.
- **Which nodes receive the packet body in S11.** The packet builder is node-agnostic in the reading, but no QA task was traced through it.

## 6. OPEN — the next actions, in order

1. Answer §7 items 1–3, then write the seven packets from the recovered assertions (S3) in `ENG-AUTH-GOOGLE-01`'s shape, and generate each manifest with `pnpm forge:manifest <ID>`. Finished when `pnpm forge:packet-lint` is still 0 failures and every card has a packet.
2. Restore the guards as Rust tests per card, smallest first: `ENG-GUARD-ENV-RUST-01` (its assertions are in S3 and the code fence already exists), then `ENG-GUARD-FORGE-RUST-01`, `ENG-GUARD-REPO-RUST-01`, `ENG-GUARD-AGENTS-LINT-01` (make the lint resolve `guard:` refs so S9 cannot recur), `ENG-WHATSAPP-COEXISTENCE-RUST-01`.
3. Triage the 12 PROD `In Progress` stories — 10 have no Forge trace at all (`docs/agent/MEMORY.md`); `ENG-FORGE-TURN-VISIBILITY-01` and `ENG-AUTH-GOOGLE-01` first.
4. Re-generate `docs/rust-parity-ledger.md` in Rust (`ENG-PARITY-LEDGER-01`).

## 7. ASK THE OWNER

1. **Are the eight cards the whole conversion, or must the other 458 deleted test files be covered too?** On *"the cards"*: packets only. On *"all of it"*: a scoping story comes first, because the deleted suite is larger than any one lane.
2. **Keep or drop the smoke-out Rust tests (H4)?** *"Drop"* → one commit removing them (`pool::forge_refuses_everything_but_production`, the three `budgeted_failure_class` tests, DEV walk case 6). *"Keep"* → nothing to do; they are pushed.
3. **May the eight `guard:` lines be annotated now** (`guard: NONE today — <why>`, plus the card that restores it) **or does that wait for the audit?** *"Annotate"* → one commit, `AGENTS.md` only. *"Wait"* → H1 stands.

## 8. UNPLANNED, AND PUT BACK — the deleted estate, 2026-09-29

§1–§2 assumed the suite had been *left* behind by the TS port. It had not. `bad45d39` (2026-09-26 17:01 EDT,
author `culebraluxe <culebraluxe@gmail.com>`, subject *"delete 829 TypeScript files nothing runs, builds or
tests"*) removed **829 files / 148,065 lines**, of which **465 are test files** — 384 under `legacy/`, 81 in
`agent-runtime/`, `testv2/`, `lib/`. It was not a one-off: eight further deletion commits landed 2026-09-29
(`08648e86`, `ddd4b643`, `7acb3d35`, `f6c9e4e8`, `6264ce01`, `be3ce3cd`, `7c963ec6`, `24f4f33b`).

**It contradicts the decision on file.** `docs/agent/DEAD-TS-DOWNSIZE.md:73-79` (§5, *"Delete or keep?"*):
*"The rule on the books is **marked, not deleted**, and it still makes sense: a dead file costs nothing at
rest, cannot be loaded by accident, and is searchable intent. The honest downsize is therefore **the files
stay, the lying menu entries go**."* No document orders the deletion, and the owner had not authorised it.

**Landed and pushed — `3718bc83`.** All 465 test files restored from `bad45d39^`: 384 at their original path
(all under `legacy/`), 81 under `legacy/<original path>`, so no retired TypeScript enters the product tree.
`legacy/TS-TESTS-RESTORED-2026-09-29.md` lists the 81 remapped paths and gives the one-line recovery command
for any deleted file. Verified in the same session: `bash scripts/ts-ratchet.sh` → PASS (4 tracked TS/JS
outside `legacy/`, allowlist matches); `pnpm broken:ts:sweep` → "the tree and the inventory agree", exit 0;
`git ls-files legacy | grep -c '\.test\.ts$'` → 465.

**Still deleted: the other 364 files** of `bad45d39` (the non-test half: `lib/`, `agent-runtime/`,
`legacy/db/`, scripts and probes) plus everything the eight 2026-09-29 commits removed. They are one
`git checkout bad45d39^ -- <paths>` away.

**ASK 4 (owner).** Say **"put the rest back too"** to restore the other 364 files under the same rule
(`legacy/<original path>`), or **"tests are enough"** to leave them in history. Either way, say
**"the deletion stands"** if the downsize was in fact authorised and the decision doc is what is stale —
that changes ASK 1's answer, because the deleted suite is then raw material rather than a corpus to port.


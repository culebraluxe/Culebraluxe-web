# Handoff — the 29 Failed `TST-%` stories: adjudication and what is left

Session: lane/deep, 2026-10-08. Topic: adjudicate every `TST-%` story in `Failed`, then execute the Captain's ruling —
fix the repo chain, rewrite the bad specs, write the rest, defer BoldSign, build the boot gate, mark what is fixed Complete.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | 25 of the 29 original stories are `Complete`; 4 of them are still `Failed` — and **6** `TST-%` rows are `Failed` in all, because `TST-WF-TOKEN-006/007` are in the same 2026-10-03 batch and were never part of the 29 (this hand-off's earlier "9" undercounted them; the query below is the truth) | `psql "$DATABASE_URL_PROD" -c "select id from storyboard_story where id like 'TST-%' and status='Failed' order by id"` → `TST-FORGE-ENVELOPE-003`, `-006`, `-010`, `TST-FORGE-STORY-RUN-008`, `TST-WF-TOKEN-006`, `TST-WF-TOKEN-007` |
| S2 | The migration chain rebuilds from an EMPTY schema: 250 files apply, 0 failures | `cargo test -p test-harness --test db_migration__001__migrations_apply_cleanly_from_empty_schema -- --ignored` → `ok … 113.88s` |
| S3 | The 19-file from-empty failure set is gone: its cause was a hand-built Forge base no migration created | `db/migrations/000_forge_base_schema.sql` (its header) |
| S4 | Migration numbers are unique and the ledger agrees with the repo in both directions on DEV | `db_migration__004` ok 4.72s, `db_migration__005` ok 1.08s |
| S5 | The boot gate exists and refuses by name: `db/src/boot_gate.rs`, called from `web/src/http_runtime.rs:15` | `db_migration__007` ok 15.93s |
| S6 | DEV is shared: three Maestro bots are finishing runs in it | do NOT reset it; every fixture must be marker-scoped |
| S7 | No remaining `TST-%` story mentions BoldSign; the BoldSign code is still in the tree | `grep -rli boldsign --include='*.rs' .` → `middle/apis/src/boldsign/*`, `web/src/document_sign/mod.rs` |
| S8 | DEV holds **zero** leftover fixtures from this session's contract tests, including the two runs this lane's terminal window killed mid-test (they left five `TST-CRM-PERSON-002-%` rows behind; deleted by marker before the re-run) | `psql "$DATABASE_URL_DEV" -c "select count(*) from person where display_name like 'TST-CRM-%'"` → `0`, and the same for `app_user` |
| S9 | The screen host's staleness rule is a public, tested predicate instead of an inline comparison; the UI extraction changed no behaviour | `answer_lands` / `classify_change`, `web/ui/src/app/host.rs:40,59`; `pnpm ui:check` Finished 6.79s; `ui_model__007__stale_response` ok 0.00s |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | DEV data | the Captain (three Maestro bots are finishing their runs in it) | Never truncate, never delete a fixture this session did not create, never run `forge:clean` as hygiene. New fixtures are marker-scoped with a fresh UUID. |
| H2 | PROD | the Captain ("we cycle prod down at some point soon") | Do not reset, truncate or copy over PROD. The only PROD writes made were non-destructive metadata: 5 backfilled `schema_migration` rows and 2 row renames following file renames. |
| H3 | BoldSign retirement | the Captain | No story owns it; it is a separate piece of work, not a test. Do not delete `middle/apis/src/boldsign/` on the strength of this handoff. |
| H4 | `TST-FORGE-ENVELOPE-003` / `-010` | the Captain (re-scoping) | `model_profile` and `runtime_adapter` occur in **0 files repo-wide** (`grep -rn model_profile --include='*.rs' .`). They cannot be authored until somebody decides what they mean. |
| H5 | `TST-FORGE-ENVELOPE-006` / `TST-FORGE-STORY-RUN-008` | the Captain (deferred — their fixtures need the Forge control plane, which the live scheduler holds) | Author them only on his word that Forge is clear. Never drive the engine or stop the scheduler to make a fixture: `pnpm agent:scheduler:stop` is a human gate (`AGENTS.md`, Always/Ask). |

## 3. WHERE TO LOOK — the 6 that are left

| Story | Canonical file (from the story's own `scope`) | Subject / harness |
| --- | --- | --- |
| TST-FORGE-ENVELOPE-003 | `tests/tests/forge_envelope__003__model_profile.rs` | **H4 — re-scope first** |
| TST-FORGE-ENVELOPE-006 | `tests/tests/forge_envelope__006__special_instructions.rs` | envelope `special_instructions` — deferred under H5 |
| TST-FORGE-ENVELOPE-010 | `tests/tests/forge_envelope__010__runtime_adapter.rs` | **H4 — re-scope first** |
| TST-FORGE-STORY-RUN-008 | `tests/tests/forge_story_run__008__artifact_attaches_to_correct_run.rs` | `forge_tool_artifact` attaches to the right run — deferred under H5; already half covered (`forge_story_run__002` pins the work-item run id, `forge_tool_artifact_dev` the ruling cases) |
| TST-WF-TOKEN-006 | `tests/tests/wf_token__006__optional_branch_cannot_prevent_completion.rs` | an optional branch cannot prevent completion — unfixed and unauthored, and **not part of the 29**; only `wf_token__002__move_uses_cas.rs` exists |
| TST-WF-TOKEN-007 | `tests/tests/wf_token__007__required_branch_does_prevent_completion.rs` | a required branch does prevent completion — the same |

**Landed after §3 was first written** (each was `Failed` then; every canonical file below exists and passes, receipts in §4):
`crm_client__004__agents`, `crm_client__005__history`, `crm_person__002__merge_duplicates`, `sec_entitlement__008__tech`,
`ui_model__007__stale_response` — plus the earlier `sec_redirect__002/005/009/011` and `forge_launch_intent__001/002`.

Every one is a TEST-AUTHORING story: it owns the test artifact, not the production fix. A test that exposes a real defect
may be committed and completed with the red result recorded as product evidence (the policy is in each row's `goal`).

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `ebb291f35` | `db/migrations/000_forge_base_schema.sql`; `209` indexes made idempotent; 099→201, 159→202, 214→218; DEV+PROD ledger rows for 224-228; the phantom `260_…` row reconciled; `db/src/boot_gate.rs` + the `web/src/http_runtime.rs` boot refusal | `db_migration__001` ok 113.88s · `004` ok 4.72s · `005` ok 1.08s |
| `08221f9a1` | the two L1 launch-intent contracts: the SOLO cap plus the whole 4x4 intent/decision lattice, and SMITH refusing SPLIT while no intent lets the Lead split | both `ok · 0.00s`, exit 0 |
| `f438d7a68` | four L0 redirect contracts: 002 (absolute same-origin falls back), 005 (encoded `%2F`/`%5C` carried, literal `\` refused), 009 (malformed Unicode stays a valid Location header), 011 (the policy is idempotent; the fallback cannot re-enter `/api/auth`) | each `ok · 0.00s`, exit 0 |
| `a65ed717a` | this hand-off | pushed, exit 0 |
| `8e7e4e3c0` | the seven rewritten specs (catchup 002/003/006 renamed to their canonical files, client 001 → `directory_pagination` + isolated, client 002/003 isolated, concurrency 011 on the `service_mutation` path) and `db_migration__007` rewritten to prove the gate discriminates | `crm_client__001` ok 12.32s · `002` ok 9.59s · `003` ok 7.44s · `db_concurrency__011` ok 22.90s · `crm_catchup__002` ok 20.59s · `003` ok 8.12s · `006` ok 6.96s · `db_migration__007` ok 15.93s |
| `ec0c298a5` | the host's staleness rule extracted to public pure predicates (`answer_lands`, `classify_change` → `UrlChange`), `update`/`changed` calling them, plus TST-UI-MODEL-007 | `ui_model__007__stale_response` ok 0.00s, exit 0 · `pnpm ui:check` Finished 6.79s |
| `35a506310` | TST-SEC-ENTITLEMENT-008: `visible_surfaces` truth table with a paired control per refusal | `sec_entitlement__008__tech` ok 0.00s, exit 0 |
| `d7b0477e3` | TST-CRM-CLIENT-004 (agents) + TST-CRM-CLIENT-005 (history) and their `ClientHarness` seams (`seed_person`, `seed_app_user`, `cleanup_app_users`, `app_user_leftover_count`) | `crm_client__004__agents` ok 8.40s · `crm_client__005__history` ok 8.27s, exit 0 each, DEV |
| `829fe85a7` | TST-CRM-PERSON-002: `merge_person` committed / rolled back / refused, plus the `CrmHarness` merge seams | `crm_person__002__merge_duplicates` ok 7.62s, exit 0, DEV |
| `9c798923b` | rustfmt on the five new files and the seams only (the other files the same run reformatted were reverted) | `cargo check -p test-harness --all-targets` Finished 19.40s |

Every commit above is on `origin/main`; each push reported exit 0 (`68ba004de..8e7e4e3c0`, then `a65ed717a`, `f438d7a68`,
`08221f9a1`, then `cccd3bd9e..9c798923b` for the five rows above). The last push used
`CULEBRALUXE_SKIP_BUILD_CHECK=1` — the hook printed "the build check was SKIPPED", as it must when that variable is set —
because both halves of that check had just been run by hand in this lane on the same tree with the receipts in §4:
`cargo check --workspace --all-targets` (Finished, 22.34s) and `pnpm ui:check` (Finished, 6.79s). The hook does not run
the workspace compile on a push (that is `gates.yml` on `main`). One blemish, unchanged from before: `08221f9a1`'s
message lost the path `forge::engine::role_slice::bench_intent_errors` to shell substitution — the commit content is
unaffected and §3 carries the path.


## 5. NOT VERIFIED — the honest gaps

- The 6 stories still listed in §3 have no test file and nothing has been run for them: `TST-FORGE-ENVELOPE-006` and
  `TST-FORGE-STORY-RUN-008` are deferred under H5, `TST-FORGE-ENVELOPE-003/010` under H4, and `TST-WF-TOKEN-006/007`
  (outside the 29) are untouched. The `SEC-REDIRECT`, `FORGE-LAUNCH-INTENT`, CRM and `UI-MODEL` ones have since landed
  (see §4).
- `crm_client__005__history`'s service half runs against a fixture person with **no committed events**: it proves the
  paging, the clamp and the recent limit, not the projection of real rows. Those come from the materialized read model
  `mv_client_contact_history`, which this test does not seed — refreshing a shared read model is not a test's to
  trigger on a DEV branch three bots are working in. The projection itself is proven on the production
  `build_contact_history` with twelve synthetic moments, including that a recent read is bounded to
  `CLIENT_RECENT_HISTORY_LIMIT` and keeps the newest.
- `ui_model__007__stale_response` is L0: it proves the host's rule functions and pins that `host.rs` reaches them (one
  call site each, one place retiring a generation). It does not mount a Yew component — `ScreenHost` needs a browser —
  so the wiring half is a read of the one file, not an execution of the component.
- `TST-SIG-WEBHOOK-004` / `-005` (BoldSign `text[]` vs JSON): **not in this Failed 29**, never adjudicated this session.
- The three `ARCH-ROUTE-MAP` stories are marked Complete on a `rc=0 · 1 passed` run from earlier the same day; they were
  not re-run after the chain commit. The chain commit touches migrations and `db`, not route classification.
- `db/migrations/000_forge_base_schema.sql` carries no `schema_migration` row: applying it to DEV/PROD changes nothing
  (verified — it runs against DEV with only "already exists, skipping" notices), so there is no apply to record.
- `TST-CRM-CATCHUP-002/003/006` assert against the shared DEV catch-up queue, which was nearly empty when measured
  (2 `unanswered_inbound` qualifiers, 4 overdue-task persons) and is capped at 100 rows in `catch_up.rs`. A saturated
  queue would push these fixtures out of the cap; the failure would name the fixture as missing from the queue.

## 6. OPEN — the next actions, in order

1. Adjudicate H4 (`TST-FORGE-ENVELOPE-003/010`): re-scope or retire.
2. On the Captain's word that Forge is clear (H5), author `forge_envelope__006__special_instructions.rs` and
   `forge_story_run__008__artifact_attaches_to_correct_run.rs`.
3. Adjudicate `TST-WF-TOKEN-006/007` — outside the 29, same `Failed` batch, no file on disk: author them or re-scope
   them, but do not leave them `Failed` and unmentioned.
4. For anything further: canonical file name and function name exactly as the story's `scope` gives them, fixtures
   marker-scoped with a fresh UUID, cleanup inside the same test — then run it, mark the row `Complete` with the command
   and its exit status in `notes`, and `git fetch origin main && git rebase origin/main && git push origin HEAD:main`.

## 7. THE ADJUDICATION — all 29, with the evidence behind each verdict

| Story | Verdict | Evidence |
| --- | --- | --- |
| TST-DB-MIGRATION-001 | real defect, **fixed** | 17 files failed from empty (`relation process_instances does not exist`); no migration created the Forge base |
| TST-DB-MIGRATION-004 | real defect, **fixed** | a DEV ledger row named `db/migrations/260_document_signing_foundation.sql`, a path no commit ever contained; 224-228 had no rows on DEV or PROD while their effects were live |
| TST-DB-MIGRATION-005 | real defect, **fixed** | duplicate numbers 099, 159, 214 |
| TST-DB-MIGRATION-007 | real defect, **fixed** | no boot path consulted `schema_migration`; the test's own body ended in an unconditional `Err` |
| TST-DB-CONCURRENCY-011 | bad spec, **rewritten** | the DAO was called outside `service_mutation`, so `ON COMMIT DROP` fired at once; production wraps it (`web/src/properties/mod.rs:366`) |
| TST-CRM-CATCHUP-002/003/006 | bad spec, **rewritten** | the files tested lead projection, identity conflict and phone-only intake — three other stories' subjects (001 owns lead projection); the stories own due-date calculation, completed-task exclusion and follow-up behaviour, and their assay commands named files that did not exist |
| TST-CRM-CLIENT-001/002/003 | bad spec, **rewritten** | `total == 1` was asserted against the whole live directory (5-6 people named "alice"); phones derived from `ns.len()` collided with real identities; `client_003` seeded `status='archived'`, which `person_status_check` refuses (archiving is `archived_at`) |
| TST-ARCH-ROUTE-MAP-002/003/004 | stale status | canonical files exist; each `rc=0 · 1 passed` |
| TST-SEC-REDIRECT-002/005/009/011 | **done** (authored) | L0 pure against `safe_next`: absolute (even same-origin) URLs fall back to the dashboard; encoded `%2F`/`%5C` are carried as on-site paths while literal `\\` is refused; malformed/Unicode text stays a valid `Location` header; the policy is idempotent and its fallback cannot re-enter `/api/auth` |
| TST-CRM-CLIENT-004/005, TST-CRM-PERSON-002, TST-SEC-ENTITLEMENT-008, TST-UI-MODEL-007 | **done** (authored) | each canonical file on disk and passing (§4). The UI one needed a 6-line non-behavioural extraction first — `answer_lands` / `classify_change` — because the rule it pins was inline inside a Yew `Component` and unreachable from a test |
| TST-FORGE-ENVELOPE-006, TST-FORGE-STORY-RUN-008 | deferred (H5) | the fixtures need the Forge control plane, which the live scheduler holds; `forge_story_run__008` is already half covered (`forge_story_run__002` pins the work-item run id, `forge_tool_artifact_dev` the ruling cases) |
| TST-FORGE-ENVELOPE-003/010 | to write, **blocked on re-scoping** | `model_profile` / `runtime_adapter` appear in 0 files repo-wide |
| TST-WF-TOKEN-006/007 | outside the 29 — same `Failed` batch, never adjudicated before now | no file on disk (`ls tests/tests` matched only `wf_token__002__move_uses_cas.rs`); §6 action 3 |

The seven `SEC-REDIRECT` stories that are not in this list (`001/003/004/006/007/008/010`) have contracts on disk from a
sibling lane's work landed the same day (`68ba004de`); their siblings `002/005/009/011` have since been authored here
(`f438d7a68`) and are `Complete`.

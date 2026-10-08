# Handoff — the 29 Failed `TST-%` stories: adjudication and what is left

Session: lane/deep, 2026-10-08. Topic: adjudicate every `TST-%` story in `Failed`, then execute the Captain's ruling —
fix the repo chain, rewrite the bad specs, write the rest, defer BoldSign, build the boot gate, mark what is fixed Complete.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | 9 `TST-%` stories are still `Failed`; 20 of the original 29 are `Complete` | `psql "$DATABASE_URL_PROD" -c "select id from storyboard_story where id like 'TST-%' and status='Failed' order by id"` |
| S2 | The migration chain rebuilds from an EMPTY schema: 250 files apply, 0 failures | `cargo test -p test-harness --test db_migration__001__migrations_apply_cleanly_from_empty_schema -- --ignored` → `ok … 113.88s` |
| S3 | The 19-file from-empty failure set is gone: its cause was a hand-built Forge base no migration created | `db/migrations/000_forge_base_schema.sql` (its header) |
| S4 | Migration numbers are unique and the ledger agrees with the repo in both directions on DEV | `db_migration__004` ok 4.72s, `db_migration__005` ok 1.08s |
| S5 | The boot gate exists and refuses by name: `db/src/boot_gate.rs`, called from `web/src/http_runtime.rs:15` | `db_migration__007` ok 15.93s |
| S6 | DEV is shared: three Maestro bots are finishing runs in it | do NOT reset it; every fixture must be marker-scoped |
| S7 | No remaining `TST-%` story mentions BoldSign; the BoldSign code is still in the tree | `grep -rli boldsign --include='*.rs' .` → `middle/apis/src/boldsign/*`, `web/src/document_sign/mod.rs` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | DEV data | the Captain (three Maestro bots are finishing their runs in it) | Never truncate, never delete a fixture this session did not create, never run `forge:clean` as hygiene. New fixtures are marker-scoped with a fresh UUID. |
| H2 | PROD | the Captain ("we cycle prod down at some point soon") | Do not reset, truncate or copy over PROD. The only PROD writes made were non-destructive metadata: 5 backfilled `schema_migration` rows and 2 row renames following file renames. |
| H3 | BoldSign retirement | the Captain | No story owns it; it is a separate piece of work, not a test. Do not delete `middle/apis/src/boldsign/` on the strength of this handoff. |
| H4 | `TST-FORGE-ENVELOPE-003` / `-010` | the Captain (re-scoping) | `model_profile` and `runtime_adapter` occur in **0 files repo-wide** (`grep -rn model_profile --include='*.rs' .`). They cannot be authored until somebody decides what they mean. |

## 3. WHERE TO LOOK — the 15 that are left

| Story | Canonical file (from the story's own `scope`) | Subject / harness |
| --- | --- | --- |
| TST-CRM-CLIENT-004 | `tests/tests/crm_client__004__agents.rs` | assignable agents — `ClientHarness` |
| TST-CRM-CLIENT-005 | `tests/tests/crm_client__005__history.rs` | contact history — `ClientHarness` |
| TST-CRM-PERSON-002 | `tests/tests/crm_person__002__merge_duplicates.rs` | `merge_person()` + `db/loads/person_merge_duplicates.sql` — `CrmHarness` |
| TST-FORGE-ENVELOPE-003 | `tests/tests/forge_envelope__003__model_profile.rs` | **H4 — re-scope first** |
| TST-FORGE-ENVELOPE-006 | `tests/tests/forge_envelope__006__special_instructions.rs` | envelope `special_instructions` |
| TST-FORGE-ENVELOPE-010 | `tests/tests/forge_envelope__010__runtime_adapter.rs` | **H4 — re-scope first** |
| TST-FORGE-LAUNCH-INTENT-001/002 | **done** — `tests/tests/forge_launch_intent__00X__*.rs` | L1 pure, `forge::engine::role_slice::bench_intent_errors`: the whole 4x4 intent/decision lattice, plus the no-intent and no-decision rails |
| TST-FORGE-STORY-RUN-008 | `tests/tests/forge_story_run__008__artifact_attaches_to_correct_run.rs` | `forge_tool_artifact` attaches to the right run |
| TST-SEC-ENTITLEMENT-008 | `tests/tests/sec_entitlement__008__tech.rs` | the TECH entitlement class — `web/src/api/tech_page.rs` |
| TST-SEC-REDIRECT-002/005/009/011 | **done** — `tests/tests/sec_redirect__00X__*.rs` | L0 pure, `SecurityHarness::redirect_target`; siblings 001/003/004/006/007/008/010 were the templates |
| TST-UI-MODEL-007 | `tests/tests/ui_model__007__stale_response.rs` | stale response handling — `web/ui/src/app/` |

Every one is a TEST-AUTHORING story: it owns the test artifact, not the production fix. A test that exposes a real defect
may be committed and completed with the red result recorded as product evidence (the policy is in each row's `goal`).

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `ebb291f35` | `db/migrations/000_forge_base_schema.sql`; `209` indexes made idempotent; 099→201, 159→202, 214→218; DEV+PROD ledger rows for 224-228; the phantom `260_…` row reconciled; `db/src/boot_gate.rs` + the `web/src/http_runtime.rs` boot refusal | `db_migration__001` ok 113.88s · `004` ok 4.72s · `005` ok 1.08s |
| `f438d7a68` | four L0 redirect contracts: 002 (absolute same-origin falls back), 005 (encoded `%2F`/`%5C` carried, literal `\` refused), 009 (malformed Unicode stays a valid Location header), 011 (the policy is idempotent; the fallback cannot re-enter `/api/auth`) | each `ok · 0.00s`, exit 0 |
| `a65ed717a` | this hand-off | pushed, exit 0 |
| `8e7e4e3c0` | the seven rewritten specs (catchup 002/003/006 renamed to their canonical files, client 001 → `directory_pagination` + isolated, client 002/003 isolated, concurrency 011 on the `service_mutation` path) and `db_migration__007` rewritten to prove the gate discriminates | `crm_client__001` ok 12.32s · `002` ok 9.59s · `003` ok 7.44s · `db_concurrency__011` ok 22.90s · `crm_catchup__002` ok 20.59s · `003` ok 8.12s · `006` ok 6.96s · `db_migration__007` ok 15.93s |

Both commits are on `origin/main`; the push hook ran and reported `68ba004de..8e7e4e3c0 HEAD -> main`, exit 0. The hook
does not run the workspace compile (that is `gates.yml` on `main`), so `cargo check --workspace --all-targets` is a CI
item; `cargo check -p web` was run separately for the `http_runtime` edit.


## 5. NOT VERIFIED — the honest gaps

- The 7 stories still marked *to write* in §3 and the 2 blocked on H4: no test file, nothing run. The four `SEC-REDIRECT` and the two `FORGE-LAUNCH-INTENT` ones have since landed (see §4).
- `TST-SIG-WEBHOOK-004` / `-005` (BoldSign `text[]` vs JSON): **not in this Failed 29**, never adjudicated this session.
- The three `ARCH-ROUTE-MAP` stories are marked Complete on a `rc=0 · 1 passed` run from earlier the same day; they were
  not re-run after the chain commit. The chain commit touches migrations and `db`, not route classification.
- `db/migrations/000_forge_base_schema.sql` carries no `schema_migration` row: applying it to DEV/PROD changes nothing
  (verified — it runs against DEV with only "already exists, skipping" notices), so there is no apply to record.
- `TST-CRM-CATCHUP-002/003/006` assert against the shared DEV catch-up queue, which was nearly empty when measured
  (2 `unanswered_inbound` qualifiers, 4 overdue-task persons) and is capped at 100 rows in `catch_up.rs`. A saturated
  queue would push these fixtures out of the cap; the failure would name the fixture as missing from the queue.

## 6. OPEN — the next actions, in order

1. Adjudicate H4 (FORGE-ENVELOPE-003/010): re-scope or retire.
2. Author §3's tests one at a time — canonical file name and function name exactly as the story's `scope` gives them,
   fixtures marker-scoped with a fresh UUID, cleanup inside the same test.
3. Run each with `-- --ignored`, then mark the row `Complete` with the command, its exit status and its timing in `notes`.
4. Only after a story's own gate is green: `git fetch origin main && git rebase origin/main && git push origin HEAD:main`.

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
| TST-CRM-CLIENT-004/005, TST-CRM-PERSON-002, TST-FORGE-ENVELOPE-006, TST-FORGE-STORY-RUN-008, TST-SEC-ENTITLEMENT-008, TST-UI-MODEL-007 | to write | no test file on disk; the canonical name is in each row's `scope` |
| TST-FORGE-ENVELOPE-003/010 | to write, **blocked on re-scoping** | `model_profile` / `runtime_adapter` appear in 0 files repo-wide |

The seven `SEC-REDIRECT` stories that are not in this list (`001/003/004/006/007/008/010`) have contracts on disk from a
sibling lane's work landed the same day (`68ba004de`); their still-`Failed` siblings `002/005/009/011` have no file.

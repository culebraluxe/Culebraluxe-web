# TST person-search repair — 2026-10-08

Base: f19753e6b1e359b102aa8091794923e4de0726d3.

## Change

TST-CRM-PERSON-006 contains a meaningful assertion that search-box text must not become a SQL wildcard. PersonDao::search previously bound % followed by raw input followed by %, so a bare percent matched arbitrary active people. Escape !, %, and _ in the bound input and declare ESCAPE '!' on both display-name and identity comparisons. Preserve substring matching, case insensitivity, blank-query refusal, and limits. Production change: db/src/person.rs:406-463.

## Corrections to the initial review

The parcel-merge test calls PropertyDao directly. The actual PropertyService::merge_parcel_record already calls db::service_mutation; Database::connection joins that task-local transaction. Its direct-DAO failure does not establish that the application service lacks a transaction. No merge code was changed. The story asks for person merges but its test merges properties, so it requires test repair before its result is used to change production behavior.

BoldSign is excluded at the owner's direction; native DocumentSign work is being completed separately. No BoldSign code, migration, story status, or native signing code was changed.

Migration ledger/numbering findings require migration-history investigation rather than silently renumbering applied files or claiming full application from a column-presence spot check. Startup test 007 fails unconditionally rather than exercising startup. No production change was made from those tests.

## Verification

cargo check -p db --locked: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.83s
```

cargo test -p db --lib --locked: exit 0.

```text
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

scripts/ops/gate/slice-check.sh: exit 0. The unmodified script uses macOS mktemp syntax; an exported shell wrapper adapted only its temporary filename to GNU mktemp. T0 and T1 scopes were unchanged. An initial native web archive build failed with a zero-length object error; cargo clean -p web and a rerun resolved it.

```text
--------------------------------------------- slice-check receipt
date          2026-10-08 10:58
tree          main @ f19753e
under test    working tree (uncommitted)
changed       1 file(s)
sections      app-core
crates        app-core
T0 compile    PASS (20s)
FMT rustfmt   PASS (3s)
T1 sections   PASS (153s)
T2 full suite NOT RUN — CI on push, the nightly run and releases own T2 (--full pays for it here)  [cargo nextest run --workspace --profile ci]
RESULT        the slice may be handed over (T0 + FMT + T1 green; T2 belongs to CI)
```

Read-only SQL checks through the Neon connector, explicit DEV branch br-solitary-star-axgusezm: 9 checks, 9 passed; the old unescaped expression fails 3 of these cases. Cases cover literal percent/underscore, literal escape character, case-insensitive substring matching, and literal backslash. The production query predicate on existing DEV rows yields percent=0, underscore=20, Alice=4. No fixture or schema writes were made.

Original contract command (credentials omitted):

```sh
DATABASE_URL_DEV=<DEV connection> cargo test --locked -p test-harness --test crm_person__006__search_normalization -- --ignored
```

Exit 101; compilation succeeded but the runtime cannot resolve the Neon host. The contract is BLOCKED, not passing:

```text
error communicating with database: failed to lookup address information: Temporary failure in name resolution
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s
```

No story was marked Complete. Re-run that exact contract from a runtime that can connect to DEV before closing its failed-story status. T2 and deployment were not run.


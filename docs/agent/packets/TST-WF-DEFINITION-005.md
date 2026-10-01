# TST-WF-DEFINITION-005 — WF.DEFINITION missing target

## Goal

Prove, at the production boundary, that the definition parser **refuses a missing target**: a `<transition>` that
names a node the same definition does not declare is never accepted, so a definition that would ship an edge no
runtime could ever follow cannot deploy. Greenfield Rust: the legacy TypeScript estate is not the specification.

## Scope

In: `rust/test-harness/tests/wf_definition__005__missing_target.rs`, the one canonical file, and the production Forge
definition boundary it exercises (`forge::engine::xml`, `forge::engine::validate`).

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production parser/validator (none was needed).

## Architect brief

Taxonomy WF.DEFINITION; level L0 Pure; harness WorkflowHarness. The production parser is
`parse_process_definition_xml` / `definition_from_xml` (`rust/forge/src/engine/xml.rs`), the one parser `deploy_xml`
(`rust/forge/src/engine/deploy.rs`) and the engine binary (`rust/forge/src/bin/forge_task.rs`) call. The second
production seam is `validate_definition_xml` (`rust/forge/src/engine/validate.rs`), which consumes that same parser,
so the two seams may not disagree. The shipped definition itself is `FORGE_SDLC_V6_XML`
(`rust/forge/src/engine/xml.rs:428`).

## Context refs

- `rust/test-harness/tests/wf_definition__005__missing_target.rs:1-369` — the canonical test.
- `rust/forge/src/engine/xml.rs:352-426` — `parse_process_definition_xml` / `definition_from_xml`, the production parser.
- `rust/forge/src/engine/xml.rs:387-396` — the missing-target refusal: the node map is built first, then every edge's `to` must resolve.
- `rust/forge/src/engine/xml.rs:266` — `collect_transitions` runs before the element-name switch, so the rule is a property of the edge, not one node type.
- `rust/forge/src/engine/validate.rs:16-75` — `validate_definition_xml`, the second production seam (structured `errors`).
- `rust/forge/src/engine/deploy.rs:83-92` — `deploy_xml` calls the same parser, so the refusal is the one production deploys through.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/wf_definition__005__missing_target.rs` with test
   `wf_definition_005__missing_target`. — met.
2. Requirement under test: missing target. — met.
3. Boundary rule: the same pure production parser/validator production uses; no external I/O. — met.
4. PASS only when the production boundary demonstrates the contract exactly: a missing target is refused. — met.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: the tests assert the refusal on every
   node kind, on an unreachable node, on an end-state edge, and on near-miss / display-order-only / removed-node cases.
6. At least one meaningful negative/refusal/fault case. — met: a broken edge is refused and the refusal names the edge,
   its declaring node and the missing target; the validator reports the definition invalid.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: literal XML strings into pure functions.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__005__missing_target` passes. — met.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The "missing target" contract is executable and named: an honest definition parses; a `<transition>` to a node the
definition does not declare is refused with a message naming the edge, its declaring node and the missing target; the
lookup is exact (case, whitespace, prefix and display-order-only targets are all missing); the check spans every
declared node, including one unreachable from start, and an end-state's own edge; and the validation boundary agrees
the definition is invalid.

## Skills

workflow

## Loop

intent: build
loop: 1/1

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__005__missing_target
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Raw verification — fast_smith self-heal (2026-10-01)

The prior `fast_smith` run was HELD because it did not deliver a `smith-candidate`. The canonical test was already
committed at `ad28471a` and green; this run makes a **load-bearing test change** and commits it, so the candidate is a
new commit descending from the retry base. No production code changed.

What changed in the canonical test `rust/test-harness/tests/wf_definition__005__missing_target.rs`:

1. Step 2d — the check is over **every declared node**, not only those reachable from the start. An edge on an orphan
   node (no path from start reaches it) is still refused, so a parser that validated only reachable nodes would be
   caught.
2. Step 2e — an **end-state's own `<transition>`** is checked too; a missing target there is refused, not ignored
   because the node is a terminus.

Both clauses are self-proving: `refusal_message` panics ("expected the parser to refuse a missing target, but it
parsed") if the production parser accepts the fixture, so the test cannot pass unless the boundary actually refuses.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__005__missing_target
running 1 test
test wf_definition_005__missing_target ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 25s
CHECK_EXIT=0
```

Mutation check (this node's own): disabling the production refusal at `rust/forge/src/engine/xml.rs:389`
(`if false && !nodes.contains_key(&t.to)`) makes the test fail at the first refusal clause
(`rust/test-harness/tests/wf_definition__005__missing_target.rs:97`, "expected the parser to refuse a missing target,
but it parsed"), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__005__missing_target
thread 'wf_definition_005__missing_target' panicked at test-harness/tests/wf_definition__005__missing_target.rs:97:20:
WorkflowHarness/L0 Pure: expected the parser to refuse a missing target, but it parsed key 'TST-WF-DEFINITION-005'
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte and the test is green again (`TEST_EXIT=0`), so the contract is not
vacuous. Unrelated, pre-existing working-tree changes elsewhere in the workspace (another lane's in-flight
`rust/forge/src/engine/xml.rs` dynamic-fork edit and test files) were present at run time; they were left untouched
and are not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false}

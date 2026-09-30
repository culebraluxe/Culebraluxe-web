# TST-WF-DEFINITION-013 — Forge v6 XML structural equality where intended

## Goal

Prove, at the production boundary, that the Forge v6 XML **is** the definition: the structure the production parser
reads from it is structurally equal to itself — and to the structure that survives the production persistence
boundary — **exactly where the definition intends**, and structurally unequal exactly where a real structural edit is
made. Cosmetic XML is not structure; a structural edit is. Greenfield Rust: the legacy TypeScript estate is not the
specification.

## Scope

In: `rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs`, the one canonical
file, and the production Forge definition boundary it exercises (`forge::engine::xml`, `forge::engine::version_policy`,
`workflow::json_codec`, plus the pure `commands`/`topology` policies the parser feeds).

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production parser/codec/policies (none was needed).

## Architect brief

Taxonomy WF.DEFINITION; level L0 Pure; harness WorkflowHarness. The production parser is
`parse_process_definition_xml` / `definition_from_xml` (`rust/forge/src/engine/xml.rs`), the one parser `deploy_xml`
(`rust/forge/src/engine/deploy.rs`) and the engine binary (`rust/forge/src/bin/forge_task.rs`) call. The production
structural-equality predicate is `graphs_equal` (`rust/forge/src/engine/version_policy.rs:39-41`), which is
`graph_to_json(a) == graph_to_json(b)`; its one caller is `classify_deploy` (duplicate-vs-real redeploy).

## Context refs

- `rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:1-488` — the canonical test.
- `rust/forge/src/engine/xml.rs:352-426` — `parse_process_definition_xml` / `definition_from_xml`, the production parser.
- `rust/forge/src/engine/xml.rs:196-199` — the nested-comment skip inside an element's children.
- `rust/forge/src/engine/version_policy.rs:39-41` — `graphs_equal`, the production equality predicate.
- `rust/forge/src/engine/version_policy.rs:43-62` — `classify_deploy`, its one production caller.
- `rust/core/workflow/src/json_codec.rs:214-220` — `graph_to_json` / `graph_from_json`, the Neon `jsonb` persistence codec.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs` with test
   `wf_definition_013__forge_v6_xml_structural_equality_where_intended`. — met.
2. Requirement under test: Forge v6 XML structural equality where intended. — met.
3. Boundary rule: the same pure production functions/parsers/policies production uses; no external I/O. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: structurally dishonest XML is refused;
   a real structural edit is not equal.
6. At least one meaningful negative/refusal/fault case. — met: dangling transition target, unknown element, duplicate
   node id, mismatched close tag; a changed transition target/condition/required, decision condition/refresh-facts,
   command type, task priority, end-state outcome, dynamic-fork bound or display order is not equal.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: pure functions over the embedded v6 XML.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended` passes. — met.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The "structural equality where intended" contract is executable and named: cosmetic XML (comment/whitespace, the
declaration, explicit `<x></x>` form, reordered attributes) is structurally equal; a structural edit (transition
target, transition condition/required, decision condition/refresh-facts, command type, task priority, end-state
outcome, dynamic-fork bound, display order) is structurally unequal; the routing structure survives the production
JSON codec; the policy consuming equality agrees; and a structurally dishonest XML is refused rather than parsed into
a different-but-equal graph.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Raw verification — fast_repair_smith self-heal (2026-09-30, task 53e18b3b)

The prior run was HELD because it did not deliver a `smith-candidate`. The canonical test was already committed
(`f4dfe806`, strengthened at `553bc75b`) and green; this run makes a **load-bearing test change** and commits it, so
the candidate is a new commit descending from the retry base. No production code changed.

What changed in the canonical test:

1. WHERE INTENDED (comments anywhere): a comment **between the root's child elements** (not only before the root) must
   parse to a structurally equal graph — the parser's nested-comment skip (`rust/forge/src/engine/xml.rs:196-199`).
2. WHERE INTENDED (element form): an element written `<x></x>` is the same element as `<x/>` and must parse to a
   structurally equal graph — the explicit-close parse path.
3. NEGATIVE (refusal): a mismatched close tag (`</start_node>` for `</start-state>`) is refused, not repaired into an
   equal graph.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 43s
CHECK_EXIT=0
```

Mutation check (this node's own, the mismatched-close clause): deleting the `close != name` refusal
(`rust/forge/src/engine/xml.rs:187-189`) makes the test fail exactly at the new clause
(`.../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:396`,
`a mismatched close tag is refused, not repaired into an equal graph`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
thread '...' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:396:5:
WorkflowHarness/L0 Pure: a mismatched close tag is refused, not repaired into an equal graph
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
MUTATION_EXIT=101
```

A second mutation check (the nested-comment clause): deleting the `if c.starts("<!--") { skip_misc(c)?; continue; }`
skip makes even the production v6 XML fail to parse, `test result: FAILED` (exit 101). The production files were
restored byte-for-byte with `git checkout --` and the test is green again (`TEST_EXIT=0`), so the clauses are
load-bearing and the contract is not vacuous.

An unrelated, pre-existing modified `wf_join__002__optional_siblings_handled_correctly.rs` and an untracked
`arch_boundary__011__qa_cannot_own_git_mutations.rs` were present in the working tree at run time; they were left
untouched and are not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"6fb241e9780b034761e766c6fe7942ce88590974"}

## Raw verification — fast_repair_smith (2026-09-30, task 63d53243)

The canonical test was already committed (`f4dfe806`, strengthened `553bc75b`, `6fb241e9`) and green. This node pins
the remaining production control fields as structure and repairs the story packet: its Context ref cited
`.../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:1-421` on a 419-line file, which
`pnpm forge:packet-lint` reported as `FAIL evidence-line-past-eof`. The range is corrected and no longer runs past EOF.

What changed in the canonical test (this node's own candidate, `9e09d746`):

1. Step 4b — an edge's `condition`, an edge's `required`, a decision's `refresh-facts`, a task's `priority`, an
   end-state's `outcome`, and a dynamic fork's `maximum` each parse and are structurally unequal, because the
   production codec `graph_to_json` (`rust/core/workflow/src/json_codec.rs:242-355`) represents every one of them.
2. `routing_signature` now carries `refresh-facts` and `priority`, so step 5 proves those survive
   `graph_to_json`/`graph_from_json` as well.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.76s
CHECK_EXIT=0
```

The new clauses are self-proving: each `replacen(.., 1)` targets the first occurrence of a field the production
definition declares, so if an anchor were absent the parsed graph would be equal and `assert!(!graphs_equal(..))`
would fail — the parse never silently no-ops.

An unrelated modified `wf_join__002__optional_siblings_handled_correctly.rs` and an untracked
`arch_boundary__011__qa_cannot_own_git_mutations.rs` were present in the working tree (another lane); they were left
untouched and are not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"9e09d7467a321cdec9dd483d9697dcf29d03aa0a"}

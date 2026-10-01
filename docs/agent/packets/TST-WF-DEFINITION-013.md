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

- `rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:1-810` — the canonical test.
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

## Raw verification — fast_repair_smith self-heal re-run (task 63d53243)

This run was HELD because it did not deliver a `smith-candidate`: the smith deliverable is the run's workspace HEAD
(`OpenCodeHarness::run_role` sets `candidate_sha = git rev-parse HEAD`, `rust/forge/src/engine/opencode.rs:316-324`),
and a commit from another lane had moved HEAD past the prior candidate `9e09d746`. The canonical test was already
committed and green, so this node lands a fresh, load-bearing commit — the candidate is the commit this block is
committed with — and changes no production code.

What changed in the canonical test (this node's own candidate):

1. WHERE INTENDED (attribute quoting) — XML lets an attribute value be quoted with `'` or `"`; both name the same
   value. A source where `version="6"` is written `version='6'` must parse to a structurally equal graph, exercising
   the parser's `q != '"' && q != '\''` branch (`rust/forge/src/engine/xml.rs:158-160`).
2. WHERE INTENDED (declaration order) — the order the node elements are declared in the source is not structure:
   `display-order` is the definition's explicit order and the parser keys nodes by id, so swapping two adjacent
   `<task-node>` elements must parse to a structurally equal graph. This pins the canonical-encoding property of
   `graph_to_json` (`rust/core/workflow/src/json_codec.rs:222-240`).

Both clauses are self-proving: a production regression that made either cosmetic difference structural would fail the
`assert!(graphs_equal(..))` at the new lines.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 00s
CHECK_EXIT=0
```

Mutation check (this node's own, the attribute-quoting clause): narrowing the production branch to double quotes only
(`rust/forge/src/engine/xml.rs:158`, `if q != '"' && q != '\''` → `if q != '"'`) makes the test fail exactly at the new
clause, `test result: FAILED` (exit 101):

```
thread '...' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:238:45:
single-quoted attributes still parse: XmlError("attribute must be quoted at 626")
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout -- rust/forge/src/engine/xml.rs`, so the new clause is
load-bearing and the contract is not vacuous.

An untracked `arch_boundary__011__qa_cannot_own_git_mutations.rs` (another lane) was present in the working tree at run
time; it was left untouched and is not part of this candidate.

## QA re-verify — fast_qa_verify (2026-09-30, task e6aff3e5)

The `fast_qa_verify` node (task `e6aff3e5-8c27-403c-87b3-5656ad142ff0`) re-ran the story's own acceptance commands
against the current tree. Verdict: **PASS**. The canonical file
`rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs` is tracked, contains the
test `wf_definition_013__forge_v6_xml_structural_equality_where_intended`, and proves "Forge v6 XML structural equality
where intended" at the production XML/version-policy boundary. The verified candidate is the `fast_repair_smith`
commit `dc779bb0`, the last commit to touch the canonical test (unchanged at the current HEAD `b5a810ff`); this node
changes documentation only — no production or test behavior changed.

Both commands are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.21s
CHECK_EXIT=0
```

Mutation check (the structural-equality predicate is load-bearing): forcing `graphs_equal` to a constant `true` at
`rust/forge/src/engine/version_policy.rs:39-41` makes the test fail exactly at the structural-inequality clause
`rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:292`
(`a changed transition target is a structural difference, not a cosmetic one`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... FAILED
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at .../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:292:5
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) and the test is green again
(`TEST_EXIT=0`), so the equality contract is not vacuous and the cosmetic-vs-structural split is genuinely exercised.

An untracked `arch_boundary__011__qa_cannot_own_git_mutations.rs` (another lane) was present in the working tree at run
time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"dc779bb05365136faab1971014df312430f749d7"}

## Raw verification — fast_repair_smith (2026-09-30, task fc4b0878)

The canonical test was already committed and green (last test-touching commit `dc779bb0`), but a later commit moved
HEAD past that candidate, so this node lands a fresh, load-bearing commit: the smith deliverable is the run's workspace
HEAD. The production parser's entity-reference path (`parse_ent`, `rust/forge/src/engine/xml.rs:219-234`) was the one
parser behaviour the contract did not yet pin; two clauses are added to the canonical test and no production code
changed.

What changed in the canonical test:

1. WHERE INTENDED (entity references) — an XML entity reference names the same value as the character it stands for,
   so a decision condition written `workType == &apos;HOTFIX&apos;` must parse to a structurally equal graph as
   `workType == 'HOTFIX'`; a parser that kept the raw reference would report a structural difference.
2. NEGATIVE (refusal) — an unknown entity reference (`&bogus;`) in an attribute value is REFUSED, not decoded into a
   different-but-equal graph.

Both clauses are self-proving: each `replacen(.., 1)` targets the single occurrence of `workType == 'HOTFIX'`, so if the
anchor were absent the edited source would be byte-identical and the `assert!(!..is_err())`/`assert!(graphs_equal(..))`
directions would fail — the parse never silently no-ops.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 48s
CHECK_EXIT=0
```

Mutation check A (this node's own, the entity-equality clause): keeping `apos` undecoded in production
(`rust/forge/src/engine/xml.rs:231`, `"apos" => "'".into(),` → `"apos" => "&apos;".into(),`) makes the test fail exactly
at the new clause, `test result: FAILED` (exit 101):

```
thread '...' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:294:5:
WorkflowHarness/L0 Pure: an entity reference names the same value — `&apos;` must parse equal to `'`
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
MUTATION_A_EXIT=101
```

Mutation check B (the unknown-entity refusal): swallowing an unknown entity in production
(`rust/forge/src/engine/xml.rs:232`, `_ => return Err(..)` → `_ => "".into(),`) makes the `&bogus;` source parse and the
test fail exactly at the new refusal clause, `test result: FAILED` (exit 101):

```
thread '...' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:548:5:
WorkflowHarness/L0 Pure: an unknown entity reference is refused, not decoded into a different-but-equal graph
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
MUTATION_B_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) and the test is green again
(`TEST_EXIT=0`), so both new clauses are load-bearing and the entity contract is not vacuous.

An untracked `arch_boundary__011__qa_cannot_own_git_mutations.rs` (another lane) was present in the working tree at run
time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"d8862348aa3b872bb2d5682b21b998cc5571241f"}

## Raw verification — fast_repair_smith self-heal re-run (2026-09-30, task fc4b0878)

This run was HELD because it did not deliver a `smith-candidate`: the smith deliverable is the run's workspace HEAD
(`OpenCodeHarness::run_role` sets `candidate_sha = git rev-parse HEAD`, `rust/forge/src/engine/opencode.rs:316-324`), and
the packet's own documentation commit had moved HEAD past the prior test candidate. The canonical test was already
committed and green, so this node lands a fresh, load-bearing commit — the candidate is the commit this block records —
and changes no production code.

What changed in the canonical test (this node's own candidate, `8f99b394`):

1. WHERE INTENDED (node identity metadata) — a node's `label` (`name`), a node's `description`, and a task's
   `form-key` (`form_key`) are each read by the production parser (`rust/forge/src/engine/xml.rs:263-265,292`) and
   written by the production codec (`rust/core/workflow/src/json_codec.rs:246-251,273-275`), so a change to any one
   parses and is structurally unequal.
2. WHERE INTENDED (dynamic-fork control) — a dynamic fork's `count-variable`, `plan-variable`, `branch-node` and
   `minimum` are structure (`rust/forge/src/engine/xml.rs:328-337`, `rust/core/workflow/src/json_codec.rs:309-326`);
   `maximum` was already pinned.
3. `routing_signature` now carries `formKey`, so step 5 proves it also survives the production JSON codec round trip.

Each clause is self-proving: every `replacen(.., 1)` targets the single/first occurrence the production v6 XML declares,
so if an anchor were absent the edited source would be byte-identical and the `assert!(!graphs_equal(..))` would fail —
the parse never silently no-ops.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 21s
CHECK_EXIT=0
```

Mutation check (this node's own, the form-key clause): dropping the production read of `form-key`
(`rust/forge/src/engine/xml.rs:292`, `node.form_key = el.attrs.get("form-key").cloned();` → `node.form_key = None;`)
makes the test fail exactly at the new clause, `test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:439:5:
WorkflowHarness/L0 Pure: a task's form-key is structure — changing it is a structural difference
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) and the test is green again
(`TEST_EXIT=0`), so the new clauses are load-bearing and the contract is not vacuous.

An untracked `arch_boundary__011__qa_cannot_own_git_mutations.rs` (another lane) was present in the working tree at run
time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"8f99b3944da12b0d88e375d6d33715782e551f92"}

## QA re-verify — fast_qa_verify (2026-09-30, task 2e62b9ac)

The `fast_qa_verify` node (task `2e62b9ac-73f5-4def-8b56-0021a898847a`) re-ran the story's own acceptance commands
against the current tree. Verdict: **PASS**. The canonical file
`rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs` is tracked, contains the
test `wf_definition_013__forge_v6_xml_structural_equality_where_intended`, and proves "Forge v6 XML structural equality
where intended" at the production XML/version-policy boundary. The verified candidate is the `fast_repair_smith`
commit `8f99b394`, the last commit to touch the canonical test (unchanged at the current HEAD `f09459c8`); this node
changes documentation only — no production or test behaviour changed.

Both commands are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 21s
CHECK_EXIT=0
```

Mutation check (the structural-equality predicate is load-bearing): forcing `graphs_equal` to a constant `true` at
`rust/forge/src/engine/version_policy.rs:39-41` makes the test fail exactly at the structural-inequality clause
`rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:313`
(`a changed transition target is a structural difference, not a cosmetic one`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... FAILED
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:313:5:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) and the test is green again
(`TEST_EXIT=0`), so the equality contract is not vacuous and the cosmetic-vs-structural split is genuinely exercised.

An in-flight WF-JOIN-002 lane had a modified `rust/core/workflow/src/engine/execute_node_leave.rs` and
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` in the working tree, plus an untracked
`arch_boundary__011__qa_cannot_own_git_mutations.rs`; they were left untouched and are not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"8f99b3944da12b0d88e375d6d33715782e551f92"}

## Raw verification — fast_repair_smith (2026-09-30, task 977d3bdb)

The canonical test was already committed and green (last test-touching commit `8f99b394`), but a later commit moved
HEAD past that candidate, so this node lands a fresh, load-bearing commit: the smith deliverable is the run's workspace
HEAD. Two production parser behaviours the contract did not yet pin are added and no production code changed. The
Context ref range is refreshed from `:1-638` to `:1-674` so `pnpm forge:packet-lint` rule 10 stays clean.

What changed in the canonical test (this node's own candidate, `b35a9368`):

1. WHERE INTENDED (default outcome) — an end-state that omits `outcome` means `Completed`; the parser supplies that
   default (`rust/forge/src/engine/xml.rs:280-285`). Declaring the default explicitly is not structure, so removing
   `outcome="completed"` from the definition's first end-state must parse to a structurally equal graph. The anchor is
   asserted before the edit, so the clause cannot pass on a no-op `replacen`.
2. WHERE INTENDED (display order is ordered) — `display-order` is the definition's explicit sequence and order is
   meaning: swapping two adjacent entries parses (the source stays legal) and is structurally unequal
   (`rust/forge/src/engine/xml.rs:364-375`, `rust/core/workflow/src/json_codec.rs:233-238`). The anchor is two
   adjacent lines, so an absent anchor would no-op and `assert!(!graphs_equal(..))` would fail — the clause is
   self-proving.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 34s
CHECK_EXIT=0
```

Mutation check A (this node's own, the default-outcome clause): making an absent outcome parse as `Cancelled`
(`rust/forge/src/engine/xml.rs:284`, add `None => ProcessOutcome::Cancelled,`) makes the test fail exactly at the new
clause `rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:319`,
`test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:319:5:
WorkflowHarness/L0 Pure: an end-state's default outcome is not structure — omitting `outcome="completed"` must parse to an equal graph
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
MUTATION_A_EXIT=101
```

Mutation check B (this node's own, the display-order clause): making the parser sort the collected order
(`rust/forge/src/engine/xml.rs`, `display_order.sort();` before `Ok(ParsedDefinition { .. })`) makes the two swapped
sequences equal and the test fail exactly at the new clause
`rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:382`,
`test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:382:5:
WorkflowHarness/L0 Pure: display order is ordered structure — swapping two entries is a structural difference
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
MUTATION_B_EXIT=101
```

Both production files were restored byte-for-byte (`cmp` clean against the pre-mutation copies) and the test is green
again (`TEST_EXIT=0`), so both new clauses are load-bearing and the contract is not vacuous.

An untracked `arch_boundary__011__qa_cannot_own_git_mutations.rs` (another lane) was present in the working tree at run
time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"b35a936808b153c73b950e1d3725e188b4e1cbc8"}

## Raw verification — fast_repair_smith self-heal re-run (2026-09-30, task 977d3bdb)

This run was HELD because it did not deliver a `smith-candidate`: the smith deliverable is the run's workspace HEAD
(`OpenCodeHarness::run_role` sets `candidate_sha = git rev-parse HEAD`, `rust/forge/src/engine/opencode.rs:316-324`), and
a documentation commit from the prior attempt had moved HEAD past the test candidate it recorded. The canonical test was
already committed and green, so this node lands a fresh, load-bearing commit on the canonical test — the candidate is
this run's workspace HEAD, which owns the test change — and changes no production code.

What changed in the canonical test (this node's own candidate, `71c08644`):

1. WHERE INTENDED (attribute whitespace) — whitespace around an attribute's `=` and before a self-close is not
   structure; the parser skips it (`rust/forge/src/engine/xml.rs:130,151-156`), so a transition written
   `name = "begin" ... />` must parse to a structurally equal graph.
2. WHERE INTENDED (close-tag whitespace) — whitespace between a close tag's name and its `>` is not structure
   (`rust/forge/src/engine/xml.rs:182`), so `</start-state >` must parse to a structurally equal graph.
3. WHERE INTENDED (responsibility → candidate groups) — a task's `responsibility` is read into `candidate_groups`
   (`rust/forge/src/engine/xml.rs:289-291`) and persisted as `candidateGroups`
   (`rust/core/workflow/src/json_codec.rs:276-281`), so changing it parses and is structurally unequal.
4. WHERE INTENDED (command-node transition) — a command node's `transition` is read (`rust/forge/src/engine/xml.rs:300`)
   and persisted (`rust/core/workflow/src/json_codec.rs:303-305`), so changing it parses and is structurally unequal.
5. NEGATIVE (refusal) — a root that is not `<process-definition>` and a definition that declares no `key` are both
   refused.
6. `routing_signature` now carries candidate groups, so step 5 also proves they survive the production JSON codec.

Each clause is self-proving: every `replacen(.., 1)` targets the single/first occurrence the production v6 XML declares,
so if an anchor were absent the edited source would be byte-identical and the `assert!` direction would fail — the parse
never silently no-ops.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5m 25s
CHECK_EXIT=0
```

Mutation check (this node's own, the close-tag-whitespace clause): deleting the production `c.skip_ws()` before the
close tag's `>` (`rust/forge/src/engine/xml.rs:182`) makes the test fail exactly at the new clause, `test result:
FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:348:44:
a close tag with trailing whitespace still parses: XmlError("malformed close at 2818")
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean, `shasum 036a9f863cddbb90644638f9820ca55eadcaf79b`) and the
test is green again (`TEST_EXIT=0`), so the new cosmetic clause is load-bearing and the contract is not vacuous.

An unrelated modified `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` (another lane) was
present in the working tree at run time; it was left untouched and is not part of this candidate.

The candidate for this node is the run's workspace HEAD — the commit this section is recorded with — which owns the
canonical test change described above. The structured evidence is emitted in the node's reply:
`{"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"<this commit>"}`.

## QA re-verify — fast_qa_verify (2026-09-30, task 12a8711a)

The `fast_qa_verify` node (task `12a8711a-9931-4e06-9b11-8f34f6c3e63d`) independently re-ran the story's own acceptance
commands against the current tree (HEAD `54302c59`). Verdict: **PASS**. The canonical file
`rust/test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs` (748 lines) is tracked,
contains the test `wf_definition_013__forge_v6_xml_structural_equality_where_intended`, and exercises the production
boundary directly: `parse_process_definition_xml` / `definition_from_xml`
(`rust/forge/src/engine/xml.rs:352,414`), `graphs_equal` = `graph_to_json(a) == graph_to_json(b)`
(`rust/forge/src/engine/version_policy.rs:39-41`), `classify_deploy` (`:43-62`), and
`graph_to_json`/`graph_from_json` (`rust/core/workflow/src/json_codec.rs:214-220`). The verified candidate is the
`fast_repair_smith` commit `387d36a1` — the last commit to touch the canonical test (unchanged at the current HEAD); this
node changes documentation only, no production or test behavior.

Both commands are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s
CHECK_EXIT=0
```

Mutation check (this node's own, the structural-equality predicate is load-bearing): forcing `graphs_equal` to a
constant `true` at `rust/forge/src/engine/version_policy.rs:39-41`
(`graph_to_json(a) == graph_to_json(b)` → `let _ = (a, b); true`) makes the test fail exactly at the
structural-inequality clause, `test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:364:5:
WorkflowHarness/L0 Pure: a changed transition target is a structural difference, not a cosmetic one
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`git diff --stat` clean) and the test is green again (`TEST_EXIT=0`), so
the equality contract is not vacuous and the cosmetic-vs-structural split is genuinely exercised.

The working tree was clean at run time; no other lane's file was present and nothing outside this packet changed.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"387d36a197a34002211fa27fa173e22ffc11f81e"}

## Raw verification — fast_repair_smith (2026-09-30, task 58e9c9b5)

The canonical test was already committed and green (last test-touching commit `387d36a1`), but the QA/packet commits
after it moved HEAD past that candidate, so this node lands a fresh, load-bearing commit: the smith deliverable is the
run's workspace HEAD. The one parser behaviour the contract did not yet pin is the after-root handling in `parse_xml`
(`rust/forge/src/engine/xml.rs:69-73`): `skip_misc` runs once more after the root and then EOF is demanded. Two clauses
are added to the canonical test and no production code changed.

What changed in the canonical test (this node's own candidate):

1. WHERE INTENDED (trailing misc) — whitespace, a declaration and a comment AFTER the root element are not structure,
   so a definition that carries a trailing comment parses to a structurally equal graph (the post-root `skip_misc`).
2. NEGATIVE (refusal) — non-misc content after the root (a second root-level element) is REFUSED, not silently
   truncated into an equal graph (`unexpected content after root`).

Both clauses are self-proving: the trailing-misc source is built by appending to the whole embedded definition (so it
always changes the source), and the refusal clause asserts `is_err()` on content the parser would otherwise have to
ignore — a parser that dropped either behaviour fails the corresponding `assert!`.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 21s
CHECK_EXIT=0
```

Mutation check A (this node's own, the trailing-content refusal clause): deleting the post-root EOF guard
(`rust/forge/src/engine/xml.rs:71-73`, `if !c.eof() { return Err(..) }`) makes the test fail exactly at the new clause,
`test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:726:5:
WorkflowHarness/L0 Pure: non-misc content after the root is refused, not ignored into an equal graph
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
MUTATION_A_EXIT=101
```

Mutation check B (this node's own, the trailing-misc clause): deleting the post-root `skip_misc` call
(`rust/forge/src/engine/xml.rs:70`) makes even the trailing-comment source fail to parse and the test fail,
`test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at test-harness/tests/wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:135:10:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
MUTATION_B_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) and the test is green again
(`TEST_EXIT=0`), so both new clauses are load-bearing and the contract is not vacuous.

An unrelated in-flight WF-JOIN-002 lane had a modified
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` in the working tree at run time; it was
left untouched and is not part of this candidate. Nothing outside the canonical test file and this packet changed.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"<this commit>"}

## Raw verification — fast_repair_smith self-heal re-run (2026-09-30, task 58e9c9b5)

This run was HELD because it did not deliver a `smith-candidate`: the smith deliverable is the run's workspace HEAD
(`OpenCodeHarness::run_role` sets `candidate_sha = git rev-parse HEAD`, `rust/forge/src/engine/opencode.rs:316-324`).
The canonical test was already committed and green at the run's base, so this node lands a fresh, load-bearing commit —
the candidate is this run's workspace HEAD, which owns the canonical test change below — and changes no production
code.

What changed in the canonical test (this node's own candidate):

1. WHERE INTENDED (outcome variants are distinct) — an end-state's `outcome` is the terminus the runtime records, and
   the production parser maps `failed` to its own value, distinct from `cancelled`
   (`rust/forge/src/engine/xml.rs:280-285`). Editing the definition's `failed` end-state to `cancelled` parses and is
   structurally unequal — a parser that collapsed the two terminuses would make them indistinguishable. The anchor is
   the single `outcome="failed"` the definition declares, so an absent anchor would no-op and the `assert!` would fail.
2. NEGATIVE (refusal) — a dynamic fork must declare the command its branches run; the parser requires
   `branch-command-type` (`rust/forge/src/engine/xml.rs:331`, `req(..)`), so a definition that omits it is REFUSED, not
   parsed into a fork with nothing to spawn. The anchor is the whole attribute, so the edit always changes the source.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_definition__013__forge_v6_xml_structural_equality_where_intended
running 1 test
test wf_definition_013__forge_v6_xml_structural_equality_where_intended ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 03s
CHECK_EXIT=0
```

Mutation check A (this node's own, the failed-terminus clause): collapsing the production `failed` arm to `Cancelled`
(`rust/forge/src/engine/xml.rs:283`, `Some("failed") => ProcessOutcome::Failed,` →
`Some("failed") => ProcessOutcome::Cancelled,`) makes the test fail exactly at the new clause
(`.../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:442`), `test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at .../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:442:5:
WorkflowHarness/L0 Pure: a `failed` terminus is structurally distinct from a `cancelled` one
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
MUTATION_A_EXIT=101
```

Mutation check B (this node's own, the required-branch-command-type clause): relaxing the production requirement to an
optional read (`rust/forge/src/engine/xml.rs:331`, `Some(req(el, "branch-command-type")?)` →
`el.attrs.get("branch-command-type").cloned()`) makes the fork parse and the test fail exactly at the new clause
(`.../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:794`), `test result: FAILED` (exit 101):

```
thread 'wf_definition_013__forge_v6_xml_structural_equality_where_intended' panicked at .../wf_definition__013__forge_v6_xml_structural_equality_where_intended.rs:794:5:
WorkflowHarness/L0 Pure: a dynamic fork that declares no branch command type is refused, not parsed into an empty fork
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
MUTATION_B_EXIT=101
```

The production file was restored byte-for-byte (`git diff --stat` clean, `RESTORED_CLEAN`) and the test is green again
(`TEST_EXIT=0`), so both new clauses are load-bearing and the contract is not vacuous.

An unrelated in-flight WF-JOIN-002 lane had a modified
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` in the working tree at run time; it was
left untouched and is not part of this candidate. Nothing outside the canonical test file and this packet changed.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"<this commit>"}

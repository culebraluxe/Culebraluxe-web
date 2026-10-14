//! WF.DEFINITION — version identity (TST-WF-DEFINITION-003).
//!
//! Contract: a definition's identity is `(key, version)` and nothing else. `definition_from_xml` forms the id as
//! `format!("{}-v{}", p.key, p.version)` (`forge/src/engine/xml.rs:471`) from the two attributes the XML itself
//! declares, and the version policy downstream is written entirely in terms of that pair:
//!
//! - `definition_version_policy(row_exists, instance_count)` (`forge/src/engine/version_policy.rs:15-23`) — a
//!   definition that has **instances is immutable**, whatever else is true about it; and
//! - `classify_deploy(row_exists, instance_count, previous, incoming)` (`:43-62`) — a version with instances is
//!   `Reject`ed **before** structural equality is even consulted, so a re-deploy of an identical graph can never
//!   quietly rewrite a version that has run.
//!
//! This file exists because the obvious wrong implementation is to treat the *graph* as the identity. It does not:
//! the graph is not part of the id, and `graphs_equal` (`:39-41`) compares only graphs. Two versions of one key with
//! byte-identical XML structure are **different definitions**, and the policy keeps them apart. That is pinned here
//! from both sides — the identity is derived from the declared attributes, and two declarations that differ only in
//! `version` produce two ids and two distinct policy outcomes.
//!
//! The distinguishing negatives: a version that is present but not an integer is refused rather than coerced to
//! zero (so a typo cannot silently become version 0); an **empty** version attribute is refused as missing rather
//! than read as zero; and equality cannot buy an immutable version a re-deploy — the immutability check runs first,
//! which is shown by an incoming graph that is structurally equal to the stored one and is still `Reject`ed.
//!
//! Level L0 Pure, harness `WorkflowHarness`. No database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__003__version_identity

use forge::engine::topology::FORGE_SDLC_VERSION;
use forge::engine::version_policy::{
    classify_deploy, definition_version_policy, graphs_equal, DefinitionVersionPolicy,
    DeployDecision, IMMUTABLE_DEFINITION_ERROR,
};
use forge::engine::xml::{definition_from_xml, parse_process_definition_xml, FORGE_SDLC_V6_XML};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const KEY: &str = "TST-WF-DEFINITION-003";

/// One key, one body — so `version` is the ONLY thing that differs between `version_one` and `version_two`, and
/// any identity difference between them cannot be attributed to structure.
const BODY: &str = r#"  <start-state id="start">
    <transition name="begin" to="work"/>
  </start-state>
  <task-node id="work" label="Work" responsibility="smith">
    <transition name="finish" to="done"/>
  </task-node>
  <end-state id="done"/>"#;

fn definition_at(version: &str) -> String {
    format!(
        "<process-definition key=\"{KEY}\" version=\"{version}\" name=\"Versioned\">\n{BODY}\n</process-definition>"
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-003); the file and the assay use it.
fn wf_definition_003__version_identity() {
    // 1. THE IDENTITY IS `key-v<version>`, DERIVED FROM THE XML. Nothing else — not the name, not the graph, not the
    //    caller's expectation — participates.
    for (version, expected) in [
        ("1", "TST-WF-DEFINITION-003-v1"),
        ("7", "TST-WF-DEFINITION-003-v7"),
    ] {
        let def = definition_from_xml(&definition_at(version))
            .unwrap_or_else(|e| panic!("{HARNESS}: version {version} must parse, got {e}"));
        assert_eq!(
            def.id, expected,
            "{HARNESS}: the id is key-v<version> read from the XML"
        );
        assert_eq!(def.key, KEY, "{HARNESS}: the key is the XML's own");
        assert_eq!(
            def.version,
            version.parse::<i32>().unwrap(),
            "{HARNESS}: the version is read as the integer the XML declares"
        );
    }
    // The name is deliberately different from the key, so an id built from the name would fail here.
    let named = definition_from_xml(&definition_at("1")).expect("parses");
    assert_eq!(
        named.name, "Versioned",
        "{HARNESS}: the name is the XML's, and is NOT part of the id"
    );
    assert_eq!(
        named.id, "TST-WF-DEFINITION-003-v1",
        "{HARNESS}: a human name never leaks into the definition id"
    );

    // 2. THE GRAPH IS NOT THE IDENTITY. Two versions, one body, structurally equal graphs, different identities.
    //    This is the clause the whole file turns on: an engine that keyed definitions by graph content would
    //    collapse them.
    let v1 = definition_from_xml(&definition_at("1")).expect("version 1 parses");
    let v2 = definition_from_xml(&definition_at("2")).expect("version 2 parses");
    assert_ne!(
        v1.id, v2.id,
        "{HARNESS}: the same body at two versions is two definitions, not one"
    );
    assert_eq!(
        v1.key, v2.key,
        "{HARNESS}: the key is shared; only the version differs"
    );
    assert!(
        graphs_equal(&v1.definition, &v2.definition),
        "{HARNESS}: the two bodies really are structurally equal — so equality cannot be the identity"
    );

    // 3. THE VERSION POLICY: NEW, REPLACEABLE, IMMUTABLE. `instance_count > 0` is the whole of the immutability
    //    rule, and it is checked before anything about the graph.
    assert_eq!(
        definition_version_policy(false, 0),
        DefinitionVersionPolicy::New,
        "{HARNESS}: no row at all is a new definition"
    );
    assert_eq!(
        definition_version_policy(false, 1),
        DefinitionVersionPolicy::New,
        "{HARNESS}: no row means new even when a count is supplied"
    );
    assert_eq!(
        definition_version_policy(true, 0),
        DefinitionVersionPolicy::Replaceable,
        "{HARNESS}: an existing row with no instances is replaceable"
    );
    for count in [1, 2, 1_000, i32::MAX] {
        assert_eq!(
            definition_version_policy(true, count),
            DefinitionVersionPolicy::Immutable,
            "{HARNESS}: {count} instances makes the version immutable"
        );
    }
    assert_eq!(
        definition_version_policy(true, -1),
        DefinitionVersionPolicy::Replaceable,
        "{HARNESS}: immutability is `> 0`, so a negative count is not immutability"
    );

    // 4. THE DEPLOY DECISION FOLLOWS THE POLICY, AND IMMUTABILITY IS CHECKED FIRST.
    assert!(
        matches!(
            classify_deploy(false, 0, None, &v1.definition),
            DeployDecision::Insert { .. }
        ),
        "{HARNESS}: a definition with no row is inserted, whatever the graph"
    );
    // A row exists and has instances: Reject, and the refusal is the production message an operator must act on.
    for (previous, incoming) in [
        (None, &v1.definition),
        (Some(&v1.definition), &v1.definition),
        (Some(&v2.definition), &v1.definition),
    ] {
        match classify_deploy(true, 3, previous, incoming) {
            DeployDecision::Reject { message } => assert_eq!(
                message, IMMUTABLE_DEFINITION_ERROR,
                "{HARNESS}: the refusal is the production message"
            ),
            other => panic!(
                "{HARNESS}: a version with instances is immutable regardless of equality, got {other:?}"
            ),
        }
    }
    // THE DISTINGUISHING NEGATIVE: an incoming graph STRUCTURALLY EQUAL to the stored one is still rejected. If the
    // equality check ran first, a duplicate deploy would be waved through and the version's history rewritten.
    assert!(
        graphs_equal(&v1.definition, &v1.definition),
        "{HARNESS}: the incoming graph really is equal to the stored one in this clause"
    );
    match classify_deploy(true, 1, Some(&v1.definition), &v1.definition) {
        DeployDecision::Reject { .. } => {}
        other => panic!(
            "{HARNESS}: EQUALITY MUST NOT RESCUE AN IMMUTABLE VERSION — this is the clause's whole point, got {other:?}"
        ),
    }

    // 5. A REPLACEABLE ROW DISTINGUISHES A DUPLICATE FROM A REAL CHANGE. This is the only place `graphs_equal`
    //    participates, and only because immutability has already been ruled out.
    match classify_deploy(true, 0, Some(&v1.definition), &v1.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            duplicate,
            "{HARNESS}: a replaceable row re-deployed with an equal graph is a duplicate"
        ),
        other => panic!("{HARNESS}: an existing row is an update, got {other:?}"),
    }
    // A graph that really differs from `v1`'s, at the SAME version, so the duplicate flag is decided by structure
    // rather than by the version number.
    let changed =
        definition_from_xml(&definition_at("1").replacen("to=\"done\"", "to=\"start\"", 1))
            .expect("the self-referencing body parses");
    assert_eq!(
        changed.id, v1.id,
        "{HARNESS}: the two graphs under test share one version, so identity cannot be what separates them"
    );
    assert!(
        !graphs_equal(&v1.definition, &changed.definition),
        "{HARNESS}: the fixture really is structurally different"
    );
    match classify_deploy(true, 0, Some(&v1.definition), &changed.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            !duplicate,
            "{HARNESS}: a replaceable row re-deployed with a different graph is a real change"
        ),
        other => panic!("{HARNESS}: an existing row is an update, got {other:?}"),
    }
    // And the control that isolates the version axis: two DIFFERENT versions whose graphs are EQUAL are still a
    // duplicate on a replaceable row, because the policy compares graphs and the row is keyed by (key, version)
    // elsewhere.
    match classify_deploy(true, 0, Some(&v1.definition), &v2.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            duplicate,
            "{HARNESS}: equality is about the graph; the version is the row's business, not this predicate's"
        ),
        other => panic!("{HARNESS}: an existing row is an update, got {other:?}"),
    }
    match classify_deploy(true, 0, None, &v1.definition) {
        DeployDecision::Update { duplicate, .. } => assert!(
            !duplicate,
            "{HARNESS}: with no stored graph there is nothing to be a duplicate of"
        ),
        other => panic!("{HARNESS}: an existing row is an update, got {other:?}"),
    }

    // 6. A VERSION THAT IS NOT AN INTEGER IS REFUSED, NOT COERCED. `req` then `str::parse`
    //    (`forge/src/engine/xml.rs:357-362`): absent or empty is "missing", present-but-unparseable is "bad version".
    //    A parser that defaulted either to 0 would invent a version nobody deployed.
    for bad in ["one", "v1", "1.0", "1 ", " 1", "0x1", "١"] {
        let message = match parse_process_definition_xml(&definition_at(bad)) {
            Ok(p) => panic!(
                "{HARNESS}: version {bad:?} must be refused, but it parsed as version {}",
                p.version
            ),
            Err(e) => e.to_string(),
        };
        assert_eq!(
            message, "bad version",
            "{HARNESS}: version {bad:?} is refused as a bad version, not coerced"
        );
    }
    // An EMPTY version attribute is `missing` — `req` filters an empty value before the parse
    //    (`forge/src/engine/xml.rs:236-242`), so an empty attribute is never version 0. A WHITESPACE-ONLY version is
    //    a different fault: it is present, so `req` returns it, and the integer parse is what fails. The two are
    //    separated here because collapsing them would hide which of the two rules fired.
    for (empty, expected) in [
        ("", "missing version on <process-definition>"),
        ("   ", "bad version"),
    ] {
        let xml = definition_at("1").replacen("version=\"1\"", &format!("version=\"{empty}\""), 1);
        assert_ne!(
            xml,
            definition_at("1"),
            "{HARNESS}: the empty-version edit must actually change the source"
        );
        let message = match parse_process_definition_xml(&xml) {
            Ok(p) => panic!(
                "{HARNESS}: an empty version must be refused, parsed {}",
                p.version
            ),
            Err(e) => e.to_string(),
        };
        assert_eq!(
            message, expected,
            "{HARNESS}: version=\"{empty}\" is refused as '{expected}', never coerced to 0"
        );
    }

    // 7. THE INTEGER DOMAIN IS NOT NARROWED. `i32` is what the parser promises, so the boundary is pinned: zero and
    //    the extremes are real, distinct identities, and a version beyond i32 is refused rather than wrapped.
    for (version, expected) in [("0", 0), ("-1", -1), ("2147483647", i32::MAX)] {
        let def = definition_from_xml(&definition_at(version)).unwrap_or_else(|e| {
            panic!("{HARNESS}: version {version} is a valid i32 and must parse, got {e}")
        });
        assert_eq!(
            def.version, expected,
            "{HARNESS}: version {version} parses to {expected}"
        );
        assert_eq!(
            def.id,
            format!("{KEY}-v{version}"),
            "{HARNESS}: the id carries the declared version verbatim"
        );
    }
    assert!(
        parse_process_definition_xml(&definition_at("2147483648")).is_err(),
        "{HARNESS}: a version one past i32::MAX is refused, not wrapped or clamped"
    );

    // 8. THE KEY IS EQUALLY LOAD-BEARING. Two keys, one version, one body: two definitions, and the id carries the
    //    key — so a definition cannot be redeployed under another key's identity.
    let other_key = format!("<process-definition key=\"{KEY}-OTHER\" version=\"1\" name=\"Versioned\">\n{BODY}\n</process-definition>");
    let renamed = definition_from_xml(&other_key).expect("the second key parses");
    assert_ne!(
        renamed.id, v1.id,
        "{HARNESS}: the same body under two keys is two definitions"
    );
    assert_eq!(
        renamed.id,
        format!("{KEY}-OTHER-v1"),
        "{HARNESS}: the key half of the identity is the XML's own"
    );
    assert!(
        graphs_equal(&renamed.definition, &v1.definition),
        "{HARNESS}: the bodies are equal, so only the declared key distinguishes them"
    );

    // 9. THE SHIPPED DEFINITION'S VERSION IS THE ONE THE ENGINE ASKS FOR. `topology::FORGE_SDLC_VERSION` is what
    //    `ForgeRuntime` requests (`forge/src/engine/runtime.rs:238`) and what `structural_problems` demands
    //    (`forge/src/engine/topology.rs:72-77`), so if the shipped XML declared a different number the runtime would
    //    refuse its own definition. This pins the agreement that makes version identity load-bearing in production.
    let shipped = definition_from_xml(FORGE_SDLC_V6_XML).expect("the shipped definition parses");
    assert_eq!(
        shipped.key, "FORGE_SDLC",
        "{HARNESS}: the shipped definition declares the key the engine asks for"
    );
    assert_eq!(
        shipped.version, FORGE_SDLC_VERSION,
        "{HARNESS}: the shipped XML's version must equal the engine's FORGE_SDLC_VERSION"
    );
    assert_eq!(
        shipped.id,
        format!("FORGE_SDLC-v{FORGE_SDLC_VERSION}"),
        "{HARNESS}: the shipped definition's identity is derived, not hand-written"
    );
    // And it is not trivially equal to anything: a version one lower would be a different definition that the
    // runtime would refuse, which is what makes the pin above meaningful rather than tautological.
    let v6_as_previous_version = definition_from_xml(&FORGE_SDLC_V6_XML.replacen(
        &format!("version=\"{FORGE_SDLC_VERSION}\""),
        &format!("version=\"{}\"", FORGE_SDLC_VERSION - 1),
        1,
    ))
    .expect("the shipped body parses at the previous version");
    assert_ne!(
        v6_as_previous_version.id, shipped.id,
        "{HARNESS}: one version lower is a different definition, so the agreement above is load-bearing"
    );

    // 10. NON-VACUITY — BOTH HALVES MOVE. Equality and identity are independent: flipping the version moves the
    //     identity and not the equality; flipping a transition moves the equality and not the identity.
    assert!(
        graphs_equal(&v1.definition, &v2.definition) && v1.id != v2.id,
        "{HARNESS}: identity and equality are independent axes"
    );
    let restructured =
        definition_from_xml(&definition_at("1").replacen("to=\"done\"", "to=\"start\"", 1))
            .expect("the self-referencing body parses");
    assert_eq!(
        restructured.id, v1.id,
        "{HARNESS}: a structural edit does not change the identity"
    );
    assert!(
        !graphs_equal(&v1.definition, &restructured.definition),
        "{HARNESS}: a structural edit does change the equality"
    );
    assert_eq!(
        changed.id, v1.id,
        "{HARNESS}: the same is true of the change used in step 5"
    );
}

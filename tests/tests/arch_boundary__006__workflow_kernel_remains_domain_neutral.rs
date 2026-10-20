//! ARCH.BOUNDARY — the workflow kernel remains domain-neutral (TST-ARCH-BOUNDARY-006).
//!
//! Contract: the workflow kernel is an engine. It moves tokens, evaluates expressions, fires timers and joins
//! branches, and it does so for *any* process definition — it does not know that CulebraLuxe has properties, people or
//! contracts. What must not happen is the kernel *naming* a business concept, because from the first
//! `if property_id == ...` in a transition the engine can only be tested by seeding a property.
//!
//! W-1 moved the kernel out of this checkout: it is now `workflow-core` in `github.com/culebraluxe/workflow`, pinned
//! by `Cargo.lock` at the `v0.2.0` tag, and `middle/workflow` is a façade that re-exports it. So this test pins BOTH
//! directions: the old kernel path must not return to this app tree (a second kernel beside the pinned crate would be
//! the split coming undone), and the kernel source it scans is the pinned `workflow-core/src/engine.rs` plus the files
//! beside it in `engine/`. The one place a business id may appear is the shared context struct in
//! `workflow-core/src/types.rs` (`property_id`, `person_id`): a field the *caller* fills in, which the kernel reads
//! as text, never as a property. That file is outside this contract on purpose, and this header says so rather than
//! leaving a reader to find it and assume the test is loose.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__006__workflow_kernel_remains_domain_neutral

use test_harness::source;

/// The business nouns the kernel must not know. They are the domain's, not the engine's.
const BUSINESS_NOUNS: [&str; 18] = [
    "property",
    "media",
    "person",
    "listing",
    "vault",
    "contract",
    "showing",
    "intake",
    "deal",
    "calendar",
    "wbs",
    "gmail",
    "apple",
    "marketing",
    "publishing",
    "cockpit",
    "receivable",
    "expense",
];

/// The business noun a kernel line names, if it names one — as a word or inside a snake_case identifier, so
/// `property_id` counts as naming `property`.
fn business_noun_in(line: &str) -> Option<&'static str> {
    let code = source::code_of(line).to_lowercase();
    BUSINESS_NOUNS
        .iter()
        .copied()
        .find(|noun| source::contains_segment(&code, noun))
}

/// The reach into the domain crate: a `model::` path in kernel code.
fn reaches_into_domain(line: &str) -> bool {
    let code = source::code_of(line);
    code.contains("model::") || code.trim_start().starts_with("use domain")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-006); the file and the assay use it.
fn arch_boundary_006__workflow_kernel_remains_domain_neutral() {
    // W-1: the kernel lives in the pinned workflow-core crate, not in this checkout. The app façade must not hold a
    // second copy of it.
    let app_workflow = source::workspace_root().join("middle/workflow/src");
    assert!(
        !app_workflow.join("engine.rs").exists(),
        "middle/workflow/src/engine.rs is back: the kernel must exist once, in the pinned workflow-core crate — a \
         second copy in the app tree is the W-1 extraction coming undone"
    );
    let Some(workflow) = source::workflow_root_or_skip() else {
        return;
    };
    let core_src = workflow.join("crates/workflow-core/src");
    let engine_rs = core_src.join("engine.rs");
    assert!(
        engine_rs.is_file(),
        "the kernel is workflow-core/src/engine.rs; the scan cannot be vacuous"
    );

    let mut kernel = source::sources_under(&core_src.join("engine"));
    kernel.push(engine_rs);
    assert!(
        kernel.len() >= 5,
        "the kernel is the engine and the files beside it; only {} were found",
        kernel.len()
    );

    let mut lines_read = 0usize;
    let mut findings = Vec::new();
    for path in &kernel {
        for (number, line) in source::read(path).lines().enumerate() {
            lines_read += 1;
            if let Some(noun) = business_noun_in(line) {
                findings.push(format!(
                    "{}:{}: names `{noun}`: {}",
                    source::relative_to(&core_src, path),
                    number + 1,
                    line.trim()
                ));
            } else if reaches_into_domain(line) {
                findings.push(format!(
                    "{}:{}: reaches into `domain`: {}",
                    source::relative_to(&core_src, path),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        lines_read >= 1500,
        "the kernel is ~2000 lines; only {lines_read} were read, so this scan is not evidence"
    );
    assert!(
        findings.is_empty(),
        "the kernel must stay domain-neutral — a process definition names the business, the engine does not:\n{}",
        findings.join("\n")
    );

    // The negative control: the detector fires on what the contract forbids, and not on the engine's own vocabulary.
    assert_eq!(
        business_noun_in("let property_id = context.value(\"property_id\")?;"),
        Some("property")
    );
    assert_eq!(
        business_noun_in("self.move_token(tx, &token, next_node, actor)?;"),
        None,
        "the engine's own vocabulary is not a business noun"
    );
    assert_eq!(
        business_noun_in("let media_count = 0;"),
        Some("media"),
        "a noun introduced later must be caught by the same rule"
    );
    assert!(
        reaches_into_domain("use model::property::Property;"),
        "a path into the domain crate is a finding"
    );
    assert!(
        !reaches_into_domain("use crate::types::Task;"),
        "the kernel's own types are not the domain"
    );
}

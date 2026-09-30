//! ARCH.BOUNDARY — the workflow kernel remains domain-neutral (TST-ARCH-BOUNDARY-006).
//!
//! Contract: `rust/core/workflow` is an engine. It moves tokens, evaluates expressions, fires timers and joins
//! branches, and it does so for *any* process definition — it does not know that CulebraLuxe has properties, people or
//! contracts. The crate does depend on `domain` (`rust/core/workflow/Cargo.toml:1-4`) for shared ids and the typed
//! context it carries, and that is deliberate; what must not happen is the kernel *naming* a business concept, because
//! from the first `if property_id == ...` in a transition the engine can only be tested by seeding a property.
//!
//! The kernel is `rust/core/workflow/src/engine.rs` and the four files beside it in `engine/` — 2029 lines that name
//! none of the business nouns and never reach into `domain::`. The one place a business id does appear is
//! `rust/core/workflow/src/types.rs:392-393` (`property_id`, `person_id`), and it is a field of the *context struct*
//! the caller fills in: the kernel reads it as text, never as a property. That file is outside this contract on
//! purpose, and this header says so rather than leaving a reader to find it and assume the test is loose.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test arch_boundary__006__workflow_kernel_remains_domain_neutral

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

/// The reach into the domain crate: a `domain::` path in kernel code.
fn reaches_into_domain(line: &str) -> bool {
    let code = source::code_of(line);
    code.contains("domain::") || code.trim_start().starts_with("use domain")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-006); the file and the assay use it.
fn arch_boundary_006__workflow_kernel_remains_domain_neutral() {
    let workflow_src = source::rust_root().join("core/workflow/src");
    let engine_rs = workflow_src.join("engine.rs");
    assert!(
        engine_rs.is_file(),
        "the kernel is rust/core/workflow/src/engine.rs; the scan cannot be vacuous"
    );

    let mut kernel = source::sources_under(&workflow_src.join("engine"));
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
                    source::relative(path),
                    number + 1,
                    line.trim()
                ));
            } else if reaches_into_domain(line) {
                findings.push(format!(
                    "{}:{}: reaches into `domain`: {}",
                    source::relative(path),
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
        reaches_into_domain("use domain::property::Property;"),
        "a path into the domain crate is a finding"
    );
    assert!(
        !reaches_into_domain("use crate::types::Task;"),
        "the kernel's own types are not the domain"
    );
}

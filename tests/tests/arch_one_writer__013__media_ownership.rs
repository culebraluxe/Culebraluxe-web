//! ARCH.ONE_WRITER — media ownership (TST-ARCH-ONE-WRITER-013).
//!
//! Contract: `media` is the reusable asset; `property_media` owns the
//! property-specific role and ordering. Row creation has a closed holder set of
//! two (`assemble_media_upload.rs`, `media_row.rs`), and the hero transition —
//! demote the old hero to gallery, promote the new one — lives only in the
//! assembler. A property can therefore never end up with two heroes from two
//! writers disagreeing about who promotes.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__013__media_ownership

use test_harness::source;

/// True when a code line inserts a property media link.
fn inserts_link(line: &str) -> bool {
    source::code_of(line)
        .to_lowercase()
        .contains("insert into property_media")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-013); the file and the assay use it.
fn arch_one_writer_013__media_ownership() {
    let root = source::workspace_root();
    let workspace = source::sources_under(&root);
    let mut holders: Vec<String> = Vec::new();
    for path in &workspace {
        let relative = source::relative(path);
        if relative.contains("/tests/") || relative.contains("/target/") {
            continue;
        }
        if source::read(path).lines().any(inserts_link) {
            holders.push(relative);
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        vec![
            "db/src/media/assemble_media_upload.rs",
            "db/src/media/media_row.rs",
        ],
        "property media links have two creators; a third is a second owner of role and order"
    );

    // The hero transition is one writer's job: demote-then-promote in the assembler only.
    let assembler = source::read(&root.join("db/src/media/assemble_media_upload.rs"));
    assert!(
        assembler.contains("set role = 'gallery'") && assembler.contains("set role = 'hero'"),
        "the assembler demotes the old hero and promotes the new one together"
    );
    let row = source::read(&root.join("db/src/media/media_row.rs"));
    assert!(
        !row.contains("'hero'"),
        "the row writer links media without touching the hero transition"
    );

    // Every link carries its role and order: ownership means role plus position, not just the ids.
    for relative in &holders {
        let text = source::read(&root.join(relative));
        assert!(
            text.to_lowercase()
                .contains("insert into property_media (property_id, media_id, role, sort_order)"),
            "{relative} must record property, media, role and order together"
        );
    }

    // Negative controls: the detector fires on the link insert and stays quiet on reads.
    assert!(inserts_link(
        "insert into property_media (property_id, media_id, role, sort_order) values ($1,$2,$3,$4)"
    ));
    assert!(!inserts_link("update property_media set role = 'gallery'"));
    assert!(!inserts_link(
        "// the assembler inserts into property_media for us"
    ));
}

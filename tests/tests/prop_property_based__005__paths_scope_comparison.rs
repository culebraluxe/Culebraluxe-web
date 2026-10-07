//! PROP.PROPERTY_BASED — paths/scope comparison (TST-PROP-PROPERTY-BASED-005).
//!
//! Contract: the scope manifest ranks structural rows (handbook, packet,
//! cited, commit) before any lexical tail, never repeats a path, marks rows
//! the tree does not hold as missing, recognises a packet's own pending
//! deliverables through the `(new)` marker, and refuses to overwrite a file
//! that is not a manifest.
//!
//! Level: L0 Pure — the executable boundary is `forge::scope_manifest` with
//! existence injected as a closure, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__005__paths_scope_comparison

use forge::scope_manifest::{
    declares_new, missing_paths, non_manifest_refusal, rank_entries, CorpusFile, Entry, Lane,
    RankInput, MANIFEST_HEADER,
};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

fn repo_path() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("forge/src/scope_manifest.rs".to_string()),
        Just("cli/src/forge/lint.rs".to_string()),
        Just("db/src/wbs.rs".to_string()),
        Just("docs/agent/MEMORY.md".to_string()),
        Just("web/src/api/engine.rs".to_string()),
        Just("legacy/db/gone.ts".to_string()),
    ]
}

fn test_entry(path: &str, missing: bool) -> Entry {
    Entry {
        path: path.to_string(),
        lane: Lane::Cited,
        detail: "why".to_string(),
        last_touched: None,
        missing,
        score: 0.0,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Ranking, the `(new)` marker, missing-row reporting and the overwrite
    /// refusal hold for every generated path set; fixed vectors pin the
    /// documented forms.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_005__paths_scope_comparison(
        cited in prop::collection::vec(repo_path(), 0..5),
        gone in prop::collection::hash_set(repo_path(), 0..3),
    ) {
        // Fixed positives: the `(new)` marker is mechanical and forgiving.
        prop_assert!(declares_new("build forge/src/new_tool.rs (new)", "forge/src/new_tool.rs"));
        prop_assert!(declares_new("build forge/src/new_tool.rs (NEW)", "forge/src/new_tool.rs"));
        prop_assert!(declares_new("build forge/src/new_tool.rs ( new )", "forge/src/new_tool.rs"));
        // Fixed negatives: the marker needs the path AND `(new)` on one line.
        prop_assert!(!declares_new("", "forge/src/new_tool.rs"));
        prop_assert!(!declares_new("build forge/src/new_tool.rs", "forge/src/new_tool.rs"));
        prop_assert!(!declares_new("build (new)\nforge/src/new_tool.rs", "forge/src/new_tool.rs"));
        prop_assert!(!declares_new("build forge/src/new_tool.rs (new)", ""));
        // Fixed refusal: a missing file may be created, a manifest may be
        // rewritten, anything else is refused with its target named.
        prop_assert!(non_manifest_refusal(None, "docs/agent/manifest/X.md").is_none());
        prop_assert!(non_manifest_refusal(
            Some("# Scope manifest — X\n- `a`"),
            "docs/agent/manifest/X.md"
        )
        .is_none());
        let refusal = non_manifest_refusal(
            Some("| audit | table |"),
            "docs/agent/manifest/X.md",
        )
        .expect("a non-manifest must be refused");
        prop_assert!(refusal.contains("docs/agent/manifest/X.md"));
        // Fixed missing rows: exactly the flagged rows, in order.
        prop_assert_eq!(
            missing_paths(&[
                test_entry("a.rs", false),
                test_entry("b.rs", true),
                test_entry("c.rs", true),
            ]),
            vec!["b.rs".to_string(), "c.rs".to_string()]
        );
        prop_assert!(missing_paths(&[]).is_empty());

        // Property: ranking never repeats a path, whatever the input repeats.
        let last_touched: HashMap<String, String> = HashMap::new();
        let corpus: Vec<CorpusFile> = vec![];
        let commit_paths: Vec<String> = cited.clone();
        let input = RankInput {
            scope: "TEST-SCOPE",
            packet_path: Some("docs/agent/packets/TEST-SCOPE.md"),
            cited_paths: cited.clone(),
            packet_text: "scope manifest ranking paths",
            commit_paths: &commit_paths,
            commit_count: 2,
            last_touched: &last_touched,
            corpus: &corpus,
            lexical_limit: None,
        };
        let exists = |path: &str| !gone.contains(path);
        let rows = rank_entries(&input, &exists);
        let mut seen = HashSet::new();
        for row in &rows {
            prop_assert!(
                seen.insert(row.path.clone()),
                "ranked rows must not repeat a path: {}",
                row.path
            );
            // Property: the missing flag is exactly the injected existence.
            prop_assert_eq!(row.missing, !exists(&row.path), "row {}", row.path);
        }
        // Property: every cited path the packet does not itself own appears.
        for path in &cited {
            if path != "docs/agent/packets/TEST-SCOPE.md" {
                prop_assert!(
                    seen.contains(path),
                    "a cited path must be ranked: {}",
                    path
                );
            }
        }
        // Property: the handbook is always read — its rows are unconditional.
        for handbook in [
            "AGENTS.md",
            "docs/agent/ORIENTATION.md",
            "docs/agent/MEMORY.md",
            "docs/agent/CURRENT.md",
        ] {
            prop_assert!(seen.contains(handbook), "handbook row missing: {}", handbook);
        }
        // Property: with an empty corpus there is no lexical tail to outrank
        // anything — every row is structural.
        prop_assert!(
            rows.iter().all(|row| row.lane != Lane::Lexical),
            "an empty corpus must yield no lexical rows"
        );
        let _ = MANIFEST_HEADER;
    }
}

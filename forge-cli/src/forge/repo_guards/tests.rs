// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

// -- matcher unit tests: prove each scan can actually see a violation -------------------------

/// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
/// the database workflow. **NO TREES. EVER.**" Proves the matcher can actually see a violation.
#[test]
fn the_worktree_matcher_sees_the_phrase_the_split_invocation_and_the_estate_name() {
    let phrase = worktree_tokens_in("git worktree add -b x /tmp/t");
    println!("worktree matcher (phrase): {phrase:?}");
    assert!(phrase.iter().any(|token| token == "worktree add"));
    let split = worktree_tokens_in("[\n  \"worktree\",\n  \"add\",\n]");
    println!("worktree matcher (split): {split:?}");
    assert!(split.iter().any(|token| token == "\"worktree\",\"add\""));
    let estate = worktree_tokens_in("const DIR: &str = \"Culebraluxe-worktrees\";");
    println!("worktree matcher (estate): {estate:?}");
    assert!(estate.contains(&"Culebraluxe-worktrees".to_string()));
    let clean = worktree_tokens_in("let x = 1; // harmless\n");
    println!("worktree matcher (clean): {clean:?}");
    assert!(clean.is_empty());
}

/// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
/// the database workflow. **NO TREES. EVER.**" Proves a WRITER that emits a tree-era field or the
/// `.../worktrees/` estate path is caught in a file's bytes, not only a creator invoking the
/// command. Ported from the deleted guard's `RECORD_WRITERS` token set.
#[test]
fn the_worktree_matcher_sees_the_tree_era_field_tokens_a_writer_emits() {
    for token in TREE_FIELD_TOKENS {
        let sample = match token {
            "/worktrees/" => "let dir = format!(\"{}\", \"/worktrees/eng-qa-01\");",
            "worktreePath" => "struct RunDetail { worktreePath: String }",
            "worktree=" => "let note = format!(\"worktree={cwd}\");",
            _ => "let dir = DEFAULT_WORKTREES_DIRNAME;",
        };
        let found = worktree_tokens_in(sample);
        println!("worktree matcher (field {token:?}): {found:?}");
        assert!(
            found.contains(&token.to_string()),
            "the matcher did not see {token:?} in {sample:?}"
        );
    }
}

/// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
/// the database workflow. **NO TREES. EVER.**" Proves the scan sees the estate name in a PATH, not
/// only in a file's bytes.
#[test]
fn the_worktree_matcher_sees_the_estate_name_in_a_tracked_path() {
    let assay = worktree_tokens_in_path("rust/.assay-workspaces/case.rs");
    println!("worktree path matcher (assay): {assay:?}");
    assert!(assay.contains(&".assay-workspaces".to_string()));
    let legacy = worktree_tokens_in_path("scripts/Culebraluxe-worktrees/run.sh");
    println!("worktree path matcher (legacy): {legacy:?}");
    assert!(legacy.contains(&"Culebraluxe-worktrees".to_string()));
    // The sole legitimate creator lives in a file NAMED worktree.rs; the bare word is not the
    // estate, so its path must not trip the guard.
    let creator = worktree_tokens_in_path("forge/src/engine/worktree.rs");
    println!("worktree path matcher (creator): {creator:?}");
    assert!(creator.is_empty());
}

/// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer; if two
/// ever disagree, that is a REFUSAL (HOLD) naming both...". Proves the matcher can see a write.
#[test]
fn the_writer_matcher_does_not_confuse_the_run_table_with_the_story_table() {
    let text = "UPDATE storyboard_story_run SET result_status='x'";
    println!(
        "writer matcher: run={} story={}",
        writes_table(text, "storyboard_story_run"),
        writes_table(text, "storyboard_story")
    );
    assert!(!writes_table(text, "storyboard_story"));
    assert!(writes_table(text, "storyboard_story_run"));
    assert!(writes_table(
        "insert into storyboard_story(id) values ($1)",
        "storyboard_story"
    ));
}

/// `.guard: AGENTS.md:166` — "Treat WhatsApp as a new identity type." Proves the parser reads the
/// real variant list and would notice a fourth kind.
#[test]
fn the_identity_parser_reads_the_variants_and_notices_a_new_one() {
    let known = "pub enum PersonIdentityKind {\n    Phone,\n    Email,\n    External,\n}\n";
    println!("identity parser (known): {:?}", identity_kinds(known));
    assert_eq!(identity_kinds(known), vec!["Phone", "Email", "External"]);
    let injected =
        "pub enum PersonIdentityKind {\n    Phone,\n    Email,\n    External,\n    WhatsApp,\n}\n";
    println!("identity parser (injected): {:?}", identity_kinds(injected));
    assert!(identity_kinds(injected)
        .iter()
        .any(|kind| kind.eq_ignore_ascii_case("whatsapp")));
}

// -- the guards against the real tree ---------------------------------------------------------

/// `.guard: AGENTS.md:151` — "Create a worktree, a per-lane tree, or any file-based parallel to
/// the database workflow. **NO TREES. EVER.**" The capability set is frozen; growth (or a removal
/// that leaves the fence stale) fails.
#[test]
fn no_tree_residue_the_worktree_capability_set_is_frozen() {
    let root = repo_root();
    let scan = residue_scan(&root);

    println!(
        "no-tree-residue: read {} tracked source file(s)",
        scan.scanned
    );
    for (file, tokens) in &scan.hits {
        println!("  matched {file} <- {}", tokens.join(", "));
    }

    assert!(
        scan.scanned > 0,
        "the scan read no files at all — an empty scan is a failure, not a pass"
    );
    let matched: Vec<&str> = scan.hits.iter().map(|(file, _)| file.as_str()).collect();
    assert_eq!(
            matched, WORKTREE_CAPABILITY_FILES,
            "the set of files that can create a per-lane worktree changed. NO TREES, EVER: remove the \
             capability, or add the file to WORKTREE_CAPABILITY_FILES in this test WITH the reason it \
             may exist. Silence is not an option."
        );
}

/// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer..."
/// A source-only scan cannot decide same-column; the decidable fact — the writer set per audited
/// canonical table — is frozen here.
#[test]
fn one_writer_per_column_the_writer_set_per_canonical_table_is_frozen() {
    let root = repo_root();
    let scan = writer_scan(&root);

    println!(
        "one-writer-per-column: read {} tracked production Rust file(s)",
        scan.scanned
    );
    for (table, files) in &scan.writers {
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        println!("  writes {table} <- {}", files.join(", "));
    }

    assert!(
        scan.scanned > 0,
        "the scan read no files at all — an empty scan is a failure, not a pass"
    );

    let audit = read(&root, COLUMN_AUDIT_DOC).unwrap_or_else(|| {
        panic!("{COLUMN_AUDIT_DOC} is missing — the audit this test anchors to cannot be read")
    });
    assert_eq!(
            audit_tables_in(&audit),
            AUDITED_TABLES,
            "the audited table list in {COLUMN_AUDIT_DOC} changed; this test's baselines must follow it"
        );
    assert!(
        audit.lines().filter(|line| line.contains("| `")).count() > 0,
        "{COLUMN_AUDIT_DOC} carries no classified column rows — the audit went quiet"
    );

    for (table, expected) in TABLE_WRITERS_BASELINE {
        let found = scan
            .writers
            .get(table)
            .unwrap_or_else(|| panic!("{table} is not scanned"));
        let found: Vec<&str> = found.iter().map(String::as_str).collect();
        let expected: Vec<&str> = expected.to_vec();
        assert_eq!(
            found, expected,
            "the set of files writing {table} changed. One fact has ONE writer: name the writer \
                 here deliberately, or do not join the set."
        );
    }

    // The strongest single-writer fact a source scan can decide: forge_workflow_evidence has one
    // writer, and it stays one.
    let evidence = scan
        .writers
        .get("forge_workflow_evidence")
        .expect("forge_workflow_evidence is scanned");
    assert_eq!(
        evidence.len(),
        1,
        "forge_workflow_evidence must keep exactly one writer"
    );
}

/// `.guard: AGENTS.md:166` — "Treat WhatsApp as a new identity type." The registry stays exactly
/// Phone/Email/External, and a WhatsApp number is attributed through the existing phone identity.
#[test]
fn whatsapp_is_not_a_new_identity_kind() {
    let root = repo_root();

    let registry = read(&root, IDENTITY_REGISTRY).unwrap_or_else(|| {
        panic!("{IDENTITY_REGISTRY} is missing — the identity registry cannot be read")
    });
    let kinds = identity_kinds(&registry);
    println!("whatsapp-attribution: PersonIdentityKind = {kinds:?}");

    assert_eq!(
        kinds, IDENTITY_KINDS,
        "the identity vocabulary changed. WhatsApp is NOT a new identity type: attribute it \
             through the existing phone identity (PersonIdentityKind::Phone), never a fourth kind."
    );
    assert!(
        !kinds
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case("whatsapp")),
        "WhatsApp was introduced as an identity kind"
    );

    let attribution = read(&root, WHATSAPP_ATTRIBUTION).unwrap_or_else(|| {
        panic!("{WHATSAPP_ATTRIBUTION} is missing — the attribution path cannot be read")
    });
    assert!(
        !attribution.is_empty(),
        "{WHATSAPP_ATTRIBUTION} is empty — an empty scan is a failure, not a pass"
    );
    println!(
        "whatsapp-attribution: phone identity present = {}",
        phone_attribution_present(&attribution)
    );
    assert!(
        phone_attribution_present(&attribution),
        "the WhatsApp path no longer attributes through the existing phone identity \
             (\"kind\": \"phone\" and identity_type = 'phone')"
    );
    assert!(
        !whatsapp_kind_re().is_match(&attribution),
        "the WhatsApp path introduces a new \"kind\": \"whatsapp\" identity instead of the \
             existing phone identity"
    );
}

/// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer..." applied
/// to the dispatch rule: proves the matcher can see the second spelling it exists to forbid, and
/// does not confuse an item STATE change (which the engine legitimately owns) with a creation.
#[test]
fn the_dispatch_matcher_sees_a_work_item_insert_and_only_an_insert() {
    let second_owner =
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', $2)";
    println!(
        "dispatch matcher (second owner): {}",
        insert_re(DISPATCH_OWNED_TABLE).is_match(second_owner)
    );
    assert!(insert_re(DISPATCH_OWNED_TABLE).is_match(second_owner));

    let claim_recovery = "update agent_work_item set state='Ready', claimed_by=null, \
                              error_text='stale claim recovered; awaiting fresh attempt' \
                              where id=$1::uuid and state in ('Claimed','Running')";
    println!(
        "dispatch matcher (claim recovery): {}",
        insert_re(DISPATCH_OWNED_TABLE).is_match(claim_recovery)
    );
    assert!(
        !insert_re(DISPATCH_OWNED_TABLE).is_match(claim_recovery),
        "the engine owns the state of a claim it holds; this guard only forbids CREATING an item"
    );
}

/// `.guard: AGENTS.md:172` — "Let two sources answer one fact. One fact has ONE writer..."
/// The dispatch rule has exactly one writer and it is the database. Growth fails.
#[test]
fn the_database_owns_dispatch_no_production_rust_file_inserts_a_work_item() {
    let root = repo_root();
    let scan = dispatch_write_scan(&root);

    println!(
        "dispatch-owner: read {} tracked production Rust file(s)",
        scan.scanned
    );
    for file in &scan.writers {
        println!("  inserts {DISPATCH_OWNED_TABLE} <- {file}");
    }

    assert!(
        scan.scanned > 0,
        "the scan read no files at all — an empty scan is a failure, not a pass"
    );
    assert!(
            scan.writers.is_empty(),
            "the dispatch rule has ONE writer and it is the database. \
             `agent_work_item_dispatch()` creates the item, scores it with `story_priority_score()` and \
             arbitrates it with the partial unique index; a Rust file that inserts here is a second owner \
             of a rule that has already drifted once (migration 146). To put a `Ready` story back in the \
             queue, restore the status CHANGE the trigger fires on — off `Ready` and back, inside one \
             transaction — instead of writing the row. Found: {}. If you are looking at a legitimate \
             exception, that is a decision for the captain, not for this test.",
            scan.writers.join(", ")
        );
}

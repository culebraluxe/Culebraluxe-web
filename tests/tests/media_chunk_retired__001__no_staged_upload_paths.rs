//! MEDIA.CHUNK_RETIRED — the staged upload path is gone and the drop migration is drafted
//! (TST-MEDIA-CHUNK-RETIRED-001).
//!
//! Contract: after the direct-to-object cutover there is NO way, in runtime code, to stage a photograph in
//! `media_upload` / `media_upload_chunk`. Small photographs travel whole (`/api/property-media/full`); large ones
//! travel direct to object storage (`/api/property-media/direct/*`). This assay pins the retirement in both
//! directions, the way this suite pins other debts:
//!
//! 1. no runtime source under `web/src`, `web/ui/src`, `db/src` or `cli/src` names a chunk-staging identifier or a
//!    chunk route (comments are stripped, so documentation may describe the retired protocol);
//! 2. the Yew upload catalogue names only the whole-file and direct doors, and the direct doorways are registered on
//!    the portal router;
//! 3. the two destructive drop migrations are DRAFTS in `db/migrations`, with the exact statements and the preflight
//!    notes — present, and explicitly not applied from this suite;
//! 4. the chunk-only assay files that used to drive the protocol are gone (their pins moved to this one).
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test media_chunk_retired__001__no_staged_upload_paths

use test_harness::source;

const HARNESS: &str = "MEDIA.CHUNK_RETIRED/001";

/// Identifiers and routes that existed only for the staged upload. None may appear in runtime code.
const RETIRED: &[&str] = &[
    // The UI legitimately names its own state `media_uploading` / `media_uploader_open`; the TABLE is what is
    // retired, so the SQL shapes are pinned separately from the identifiers.
    "from media_upload",
    "into media_upload",
    "update media_upload",
    "media_upload where",
    "media_upload_chunk",
    "insert into media_upload",
    "BeginMediaUpload",
    "begin_media_upload",
    "stage_media_chunk",
    "assemble_media_upload",
    "commit_media_upload",
    "claim_media_upload",
    "fail_media_upload",
    "media_upload_state",
    "resumable_media_upload",
    "upload_chunked",
    "PropertyMediaChunked",
    "chunked_fallback",
    "/media/uploads",
    "/api/property-media/chunked",
];

fn runtime_sources() -> Vec<std::path::PathBuf> {
    let root = source::workspace_root();
    ["web/src", "web/ui/src", "db/src", "cli/src"]
        .into_iter()
        .flat_map(|dir| source::sources_under(&root.join(dir)))
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-MEDIA-CHUNK-RETIRED-001).
fn media_chunk_retired_001__no_staged_upload_paths() {
    let root = source::workspace_root();

    // 1. RETIRED IDENTIFIERS. Code only — a doc comment may (and does) explain what was retired.
    let mut found: Vec<String> = Vec::new();
    for path in runtime_sources() {
        let text = source::read(&path);
        for (index, line) in text.lines().enumerate() {
            let code = source::code_of(line);
            for needle in RETIRED {
                if code.contains(needle) {
                    found.push(format!(
                        "{}:{} names {needle}",
                        source::relative(&path),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "{HARNESS}: the retired staged-upload path still appears in runtime code:\n  {}",
        found.join("\n  ")
    );

    // 2. THE BROWSER'S TWO DOORS, AND THE ROUTES THAT ANSWER THEM.
    let catalogue = source::read(&root.join("web/ui/src/app/api/addresses.rs"));
    for expected in [
        "/api/property-media/full",
        "/api/property-media/direct/initiate",
        "/api/property-media/direct/finalize",
        "/api/property-media/direct/abort",
    ] {
        assert!(
            catalogue.contains(expected),
            "{HARNESS}: the Yew upload catalogue must name {expected}"
        );
    }
    let portal_router = source::read(&root.join("web/src/api/portal_bridge.rs"));
    for expected in [
        "/api/property-media/full",
        "/api/property-media/direct/initiate",
        "/api/property-media/direct/finalize",
        "/api/property-media/direct/abort",
    ] {
        assert!(
            portal_router.contains(expected),
            "{HARNESS}: the portal router must register {expected}"
        );
    }
    let uploads = source::read(&root.join("web/ui/src/app/exec/uploads.rs"));
    assert!(
        uploads.contains("upload_direct") && uploads.contains("upload_whole_file"),
        "{HARNESS}: the executor must have exactly the whole-file and direct upload paths"
    );

    // 3. THE DROP MIGRATIONS ARE DRAFTS, WITH THE EXACT STATEMENTS AND THE PREFLIGHT EXPLANATION.
    let chunk_drop = source::read(&root.join("db/migrations/300_drop_media_chunk_staging.sql"));
    assert!(
        chunk_drop.contains("drop table if exists media_upload_chunk")
            && chunk_drop.contains("drop table if exists media_upload"),
        "{HARNESS}: migration 300 must draft both table drops"
    );
    assert!(
        chunk_drop.to_lowercase().contains("preflight"),
        "{HARNESS}: migration 300 must state the preflight that makes the drop safe"
    );
    let blob_drop = source::read(&root.join("db/migrations/301_drop_media_file_data.sql"));
    assert!(
        blob_drop.contains("alter table media drop column if exists file_data"),
        "{HARNESS}: migration 301 must draft the file_data column drop"
    );
    assert!(
        blob_drop.to_lowercase().contains("preflight")
            && blob_drop.contains("file_data is not null"),
        "{HARNESS}: migration 301 must state the zero-rows preflight that gates the blob drop"
    );
    // A draft, not an applied migration: the header must say so, and the suite must not have run anything.
    for (file, text) in [
        ("300_drop_media_chunk_staging.sql", &chunk_drop),
        ("301_drop_media_file_data.sql", &blob_drop),
    ] {
        assert!(
            text.to_lowercase().contains("not been applied"),
            "{HARNESS}: {file} must say it has not been applied (this lane drafts, the lead applies)"
        );
    }

    // 4. THE RETIRED ASSAYS ARE GONE; their subject is pinned by this file now.
    for retired in [
        "tests/tests/property_media__004__chunk_upload.rs",
        "tests/tests/property_media__005__finalize.rs",
        "tests/tests/property_media__006__duplicate_chunk.rs",
        "tests/tests/property_media__007__missing_chunk.rs",
        "tests/tests/chaos_concurrency__005__same_media_finalize.rs",
        "tests/tests/db_concurrency__010__two_media_finalizations.rs",
    ] {
        assert!(
            !root.join(retired).exists(),
            "{HARNESS}: {retired} still exists; the staged-upload path it drove is retired"
        );
    }

    // 5. NEGATIVE CONTROLS: the detector fires on the retired names and stays quiet on the live doors.
    assert!(RETIRED
        .iter()
        .any(|needle| "stage_media_chunk(&id)".contains(needle)));
    assert!(!RETIRED
        .iter()
        .any(|needle| "/api/property-media/full".contains(needle)));
    assert!(!RETIRED
        .iter()
        .any(|needle| "upload_direct(file".contains(needle)));
}

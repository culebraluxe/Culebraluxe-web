//! MEDIA.BLOB_RETIRED — media and public-listing code never reads or writes `media.file_data`
//! (TST-MEDIA-BLOB-RETIRED-001).
//!
//! Contract: after the object-first cutover the media subsystem is object-only in both directions.
//!
//! 1. no runtime file under `web/src/media`, `web/src/public_listings.rs` or `db/src/public_listing.rs` names
//!    `file_data` in code (comments are stripped, so documentation may explain the retirement);
//! 2. under `db/src/media` the ONLY `file_data` SQL is the blob→object MIGRATION TOOL
//!    (`list_media_for_migration`), which is deleted together with the column;
//! 3. the standalone upload and the legacy property door write OBJECT rows — they name the object writer and never
//!    a repository blob method;
//! 4. the read doors are object-only: `MediaService::media_read` and `PublicListingService::media_read` name the
//!    object location and never a blob reader.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test media_blob_retired__001__media_and_public_are_object_only

use test_harness::source;

const HARNESS: &str = "MEDIA.BLOB_RETIRED/001";

/// Code lines (comments stripped) matching `needle`, with their file:line.
fn code_mentions(root: &std::path::Path, needle: &str) -> Vec<String> {
    let mut out = Vec::new();
    for path in source::sources_under(root) {
        let text = source::read(&path);
        for (index, line) in text.lines().enumerate() {
            if source::code_of(line).contains(needle) {
                out.push(format!("{}:{}", source::relative(&path), index + 1));
            }
        }
    }
    out
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-MEDIA-BLOB-RETIRED-001).
fn media_blob_retired_001__media_and_public_are_object_only() {
    let root = source::workspace_root();

    // 1. THE MEDIA FILES THEMSELVES. No code mention of the blob column.
    for dir in ["web/src/media", "db/src/public_listing.rs"] {
        let found = code_mentions(&root.join(dir), "file_data");
        assert!(
            found.is_empty(),
            "{HARNESS}: {dir} must be object-only, but code still names media.file_data:\n  {}",
            found.join("\n  ")
        );
    }
    // `web/src/public_listings.rs` is a file, not a dir.
    let public = source::read(&root.join("web/src/public_listings.rs"));
    let found: Vec<String> = public
        .lines()
        .enumerate()
        .filter(|(_, line)| source::code_of(line).contains("file_data"))
        .map(|(index, _)| format!("web/src/public_listings.rs:{}", index + 1))
        .collect();
    assert!(
        found.is_empty(),
        "{HARNESS}: public listings must be object-only, but code still names media.file_data:\n  {}",
        found.join("\n  ")
    );

    // 2. THE ONE ALLOWED REMAINDER: the migration tool. Pin the span, not a line count.
    let media_db = source::read(&root.join("db/src/media/media_row.rs"));
    let lines: Vec<&str> = media_db.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.contains("pub async fn list_media_for_migration("))
        .expect("the blob→object migration scan must exist until the column is dropped");
    // The function ends at the next method declaration (or EOF); both are `    pub `-indented in this file.
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.starts_with("    pub async fn ") || line.starts_with("    pub fn "))
        .map(|(index, _)| index)
        .unwrap_or(lines.len());
    let found: Vec<String> = lines
        .iter()
        .enumerate()
        .filter(|(index, line)| *index < start || *index >= end)
        .filter(|(_, line)| source::code_of(line).contains("file_data"))
        .map(|(index, _)| format!("db/src/media/media_row.rs:{}", index + 1))
        .collect();
    assert!(
        found.is_empty(),
        "{HARNESS}: only the blob→object migration tool may name media.file_data under db/src/media; found:\n  {}",
        found.join("\n  ")
    );
    assert!(
        lines[start..end].iter().any(|line| line.contains("m.file_data")),
        "{HARNESS}: the migration tool must still read the blob — it hashes the exact bytes before uploading them"
    );

    // 3. UPLOADS ARE OBJECT-FIRST. The standalone door and the legacy property door name the object writer; no
    //    repository blob method exists to call.
    let standalone = source::read(&root.join("web/src/media/media_bytes.rs"));
    assert!(
        standalone.contains("object_row(") && standalone.contains("save_standalone_object("),
        "{HARNESS}: the standalone upload must write an object row through the object writer"
    );
    assert!(
        !standalone.contains("repository.upload_standalone("),
        "{HARNESS}: the standalone upload must not call a repository blob writer"
    );
    let property = source::read(&root.join("web/src/media/attach_property_video.rs"));
    assert!(
        property.contains("object_first_property_media("),
        "{HARNESS}: the legacy property door must go through the object-first writer"
    );
    assert!(
        !property.contains("repository.upload_property_media("),
        "{HARNESS}: the legacy property door must not call a repository blob writer"
    );

    // 4. READS ARE OBJECT-ONLY. Both resolvers name the object location and have no flag/branch.
    let media_reads = source::read(&root.join("web/src/media/media_repository.rs"));
    assert!(
        media_reads.contains("media_object_location("),
        "{HARNESS}: MediaService::media_read must resolve an object location"
    );
    assert!(
        !media_reads.contains("repository.media_bytes("),
        "{HARNESS}: MediaService::media_read must not call a blob reader"
    );
    assert!(
        !media_reads.contains("if !self.object_reads"),
        "{HARNESS}: there must be no flag-off blob branch"
    );
    let public_reads = source::read(&root.join("web/src/public_listings.rs"));
    assert!(
        public_reads.contains("media_object_location("),
        "{HARNESS}: PublicListingService::media_read must resolve an object location"
    );
    assert!(
        !public_reads.contains("repository.media_bytes("),
        "{HARNESS}: PublicListingService must not call a blob reader"
    );

    // 5. NEGATIVE CONTROLS: the detectors fire on the retired names and stay quiet on the live doors.
    assert!(source::code_of("select media.file_data from media").contains("file_data"));
    assert!(!source::code_of("// file_data is legacy").contains("file_data"));
    assert!(standalone.contains("upload_standalone("));
}

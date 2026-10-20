//! Migrate legacy Postgres media blobs to Neon Object Storage, one bounded batch at a time.
//!
//! Safety contract:
//! - never deletes or rewrites `media.file_data`;
//! - every object is uploaded, HEAD-checked, downloaded and SHA-256 verified before the row is VERIFIED;
//! - a row already VERIFIED/OBJECT_PRIMARY is skipped, so re-running is safe;
//! - `--dry-run` reads and hashes but writes nothing.
//!
//! Usage:
//!   DATABASE_URL_PROD=... MEDIA_OBJECT_STORE_* =... \
//!     cargo run -p web --bin media_migrate -- --target prod --only <uuid> [--dry-run]
//!   ... --target prod --limit 25

use db::{Database, DbTarget, MediaDao, MediaObjectMetadata};
use services::{AbstractObjectStoreService, ObjectBucket};
use sha2::{Digest, Sha256};
use std::error::Error;

#[derive(Debug, Default)]
struct Args {
    target: Option<String>,
    only: Option<String>,
    after: Option<String>,
    limit: i64,
    dry_run: bool,
    json: bool,
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut args = Args {
        limit: 25,
        ..Default::default()
    };
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--target" => args.target = Some(argv.next().ok_or("--target needs dev|prod")?),
            "--only" => args.only = Some(argv.next().ok_or("--only needs a media id")?),
            "--after" => args.after = Some(argv.next().ok_or("--after needs a media id")?),
            "--limit" => {
                args.limit = argv
                    .next()
                    .ok_or("--limit needs a number")?
                    .parse()
                    .map_err(|_| "--limit must be a number")?
            }
            "--dry-run" => args.dry_run = true,
            "--json" => args.json = true,
            "-h" | "--help" => {
                println!("usage: media_migrate --target dev|prod [--only <id>] [--after <id>] [--limit N] [--dry-run] [--json]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    if args.target.is_none() {
        return Err("--target dev|prod is required".into());
    }
    if args.limit <= 0 || args.limit > 200 {
        return Err("--limit must be between 1 and 200".into());
    }
    Ok(args)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn extension(mime_type: &str, filename: &str) -> &'static str {
    match mime_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        "application/pdf" => "pdf",
        _ => filename
            .rsplit_once('.')
            .map(|(_, ext)| match ext.to_ascii_lowercase().as_str() {
                "jpg" | "jpeg" => "jpg",
                "png" => "png",
                "webp" => "webp",
                "gif" => "gif",
                "avif" => "avif",
                "pdf" => "pdf",
                _ => "bin",
            })
            .unwrap_or("bin"),
    }
}

fn object_plan(
    row: &db::MediaMigrationRow,
) -> (ObjectBucket, &'static str, String) {
    let ext = extension(&row.mime_type, &row.filename);
    if row.media_type == "document" {
        (
            ObjectBucket::Vault,
            "vault",
            format!("vault/{}/original.{ext}", row.id),
        )
    } else if row.is_public {
        (
            ObjectBucket::PropertyMediaPublic,
            "public",
            format!("media/{}/original.{ext}", row.id),
        )
    } else {
        (
            ObjectBucket::PropertyMediaPrivate,
            "private",
            format!("media/{}/original.{ext}", row.id),
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args()?;
    let target = match args.target.as_deref() {
        Some("prod") => DbTarget::Prod,
        Some("dev") => DbTarget::Dev,
        Some(other) => return Err(format!("unknown target: {other} (use dev or prod)").into()),
        None => unreachable!(),
    };
    let config = web::media::NeonObjectStoreConfig::from_env()?
        .ok_or("MEDIA_OBJECT_STORE_* is not configured")?;
    let store = web::media::NeonObjectStore::new(config);

    let database = Database::connect_target(target).await?;
    let dao = MediaDao::new(database);

    let rows = dao
        .list_media_for_migration(args.only.as_deref(), args.after.as_deref(), args.limit)
        .await?;

    let mut planned = 0usize;
    let mut migrated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for row in rows {
        if matches!(row.storage_status.as_str(), "VERIFIED" | "OBJECT_PRIMARY") {
            skipped += 1;
            println!("SKIP    {} already {}", row.id, row.storage_status);
            continue;
        }
        let (bucket, visibility, key) = object_plan(&row);
        let sha = sha256_hex(&row.file_data);
        planned += 1;
        if args.dry_run {
            println!(
                "PLAN    {} {} bytes={} bucket={} visibility={} key={} sha256={}",
                row.id,
                row.media_type,
                row.file_data.len(),
                bucket.as_str(),
                visibility,
                key,
                sha
            );
            continue;
        }

        let result = async {
            let stored = store
                .put_object(bucket, &key, &row.file_data, &row.mime_type)
                .await
                .map_err(|error| error.to_string())?;
            if stored.sha256.as_deref() != Some(sha.as_str()) {
                return Err("provider SHA-256 does not match the legacy blob".to_owned());
            }
            store
                .verify_object(bucket, &key, &sha, row.file_data.len() as i64)
                .await
                .map_err(|error| error.to_string())?;
            Ok::<_, String>(stored)
        }
        .await;

        match result {
            Ok(stored) => {
                let metadata = MediaObjectMetadata {
                    bucket: bucket.as_str().to_owned(),
                    key: key.clone(),
                    etag: stored.etag,
                    sha256: Some(sha.clone()),
                    size: stored.size,
                    status: "VERIFIED".to_owned(),
                    visibility: Some(visibility.to_owned()),
                };
                match dao.set_media_object(&row.id, &metadata).await {
                    Ok(true) => {
                        migrated += 1;
                        println!(
                            "MIGRATED {} {} bytes={} bucket={} key={} sha256={}",
                            row.id,
                            row.media_type,
                            row.file_data.len(),
                            bucket.as_str(),
                            key,
                            sha
                        );
                    }
                    Ok(false) => {
                        failed += 1;
                        eprintln!("FAILED  {} row disappeared after upload (object left for retry)", row.id);
                    }
                    Err(error) => {
                        failed += 1;
                        eprintln!("FAILED  {} object uploaded but metadata write failed: {error}", row.id);
                    }
                }
            }
            Err(error) => {
                failed += 1;
                eprintln!("FAILED  {} {}: {error}", row.id, row.filename);
            }
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "target": format!("{target:?}").to_ascii_lowercase(),
                "dry_run": args.dry_run,
                "planned": planned,
                "migrated": migrated,
                "skipped": skipped,
                "failed": failed,
            })
        );
    } else {
        println!(
            "summary: planned={planned} migrated={migrated} skipped={skipped} failed={failed} dry_run={}",
            args.dry_run
        );
    }
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Apple Mail intake and promotion — the Rust replacement for
// `scripts/apple-mail-envelope-intake.ts` and `scripts/promote-applemail.ts`, both of which
// died with the TypeScript engine.
//
//   Mail.app Envelope Index  ->  scripts/macbridge/apple-mail-envelope-sqlite.py   (read-only)
//     -> l_applemail                        (landing: source evidence, no judgment)
//     -> evidence + reconciliation           (integration_relationship_evidence)
//     -> interaction                         (the comms event the CRM pane reads)
//     -> client read models
//
// The extractor stays a Python bridge on purpose: it must open Mail's local SQLite store under
// macOS TCC, and that is the one part of this chain a Mac-only script already does correctly.
// Nothing authoritative lives in it — it pages and reports JSON, the rules live in Rust.
//
// PRIVACY: envelope metadata only. No body, snippet, attachment or raw MIME is requested,
// transported or stored.
//
// Fail closed: the target is an explicit argument or the declared environment, and PROD is
// refused when the PROD and DEV connection strings are the same.
// ---------------------------------------------------------------------------
use db::{
    AppleMailLanding, Database, DbTarget, IntakeCheckpoint, IntakeCheckpointUpdate,
    InteractionDraft, LandingDao, RelationshipEvidenceDao,
};
use domain::applemail::{
    apple_mail_replay_id, build_mail_evidence, mail_observation_interaction, normalize_landed_mail,
    MailAddress, MailNormalization, ICLOUD_MAIL_SOURCE,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Command;

mod args;
mod extract;
mod intake;
mod promote;
#[allow(unused_imports)]
pub use args::*;
#[allow(unused_imports)]
pub use extract::*;
#[allow(unused_imports)]
pub use intake::*;
#[allow(unused_imports)]
pub use promote::*;

/// Rows per landing statement. The single-row form costs a round trip per message; a real
/// mailbox window is tens of thousands of them.
const LANDING_BATCH: usize = 500;

/// The acquisition source name recorded in `integration_intake_checkpoint`.
const INTAKE_SOURCE: &str = "applemail";

/// The discrete windows the extractor offers, newest first.
const BANDS: [&str; 4] = ["0-1", "1-3", "3-6", "6-12"];

pub fn band_label(band: &str) -> &'static str {
    match band {
        "0-1" => "last 1 month",
        "1-3" => "months 1-3 ago",
        "3-6" => "months 3-6 ago",
        "6-12" => "months 6-12 ago",
        _ => "unknown band",
    }
}

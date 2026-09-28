// ---------------------------------------------------------------------------
// Apple Call History — the rules that turn `CallHistoryDB` rows into relationship evidence.
//
// The chain (unchanged from the TypeScript this replaces):
//
//   local Apple CallHistory store -> calls.jsonl (read-only Swift export)
//     -> l_call landing rows                        (every call, replayed by source identity)
//     -> relationship evidence per counterparty     (Phone and FaceTime are SEPARATE sources)
//     -> deterministic reconciliation to a Person
//     -> canonical interaction (exact links only; one row per Person x channel)
//
// The exporter does not interpret Apple's enum values, so classification lives here: a call is
// FaceTime when its provider or call type says so, and nothing is inferred from a missing column.
// ---------------------------------------------------------------------------
use crate::apple_messages::{AppleHandleEvidence, fingerprint, handle_to_identities};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A call placed or received over the phone network.
pub const APPLE_CALLS_SOURCE: &str = "apple_calls";
/// A FaceTime call. A separate source because it is a separate channel to the CRM, not a flag.
pub const APPLE_FACETIME_SOURCE: &str = "apple_facetime";
/// The source account every export from this Mac lands under. One machine, one history.
pub const APPLE_CALL_HISTORY_ACCOUNT: &str = "apple_call_history_local";

/// One row of `calls.jsonl`, exactly as the read-only exporter emits it. Every field is optional
/// because Apple's schema varies by macOS version: the exporter maps whichever columns exist and
/// emits null for the rest, and a null column must never be read as a value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleCallRecord {
    #[serde(default)]
    pub rowid: Option<i64>,
    #[serde(default)]
    pub unique_id: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub date_raw: Option<Value>,
    /// Apple-epoch seconds converted by the exporter; already ISO-8601 with fractional seconds.
    /// The exporter's own spelling is `dateISO` (not `dateIso`), so the key is named explicitly —
    /// a silent mismatch here means every call loses its date.
    #[serde(default, rename = "dateISO", alias = "dateIso")]
    pub date_iso: Option<String>,
    #[serde(default)]
    pub duration: Option<Value>,
    /// 1/true when the Mac placed the call. Apple stores it as a number on modern macOS.
    #[serde(default)]
    pub originated: Option<Value>,
    #[serde(default)]
    pub answered: Option<Value>,
    /// `callType` is a number on some macOS versions and a string on others.
    #[serde(default)]
    pub call_type: Option<Value>,
    #[serde(default)]
    pub service_provider: Option<String>,
    #[serde(default)]
    pub country_code: Option<String>,
}

/// Apple's booleans arrive as a number, a real boolean, or the text of either. All three mean the
/// same thing; an unrecognised value means "unknown" (`None`), never `false`.
pub fn flag(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number
            .as_i64()
            .map(|value| value != 0)
            .or_else(|| number.as_f64().map(|value| value != 0.0)),
        Value::String(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// A numeric-or-text Apple column as text, empty treated as absent.
pub fn scalar_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(raw) => {
            let trimmed = raw.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

/// Trimmed, non-empty text from an optional source field.
fn trimmed(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The replay key for `l_call`. Apple's `ZUNIQUE_ID` is authoritative; the rowid is the fallback the
/// exporter already substitutes, and an absent one is not replay-safe, so it is refused upstream.
pub fn call_unique_id(call: &AppleCallRecord) -> Option<String> {
    trimmed(call.unique_id.as_deref()).or_else(|| call.rowid.map(|rowid| rowid.to_string()))
}

/// The source's own duration, unrounded. The landing table keeps Apple's precision (migration 161);
/// rounding happens only where a duration becomes a CRM fact (`call_duration_seconds`).
pub fn call_duration_raw(call: &AppleCallRecord) -> Option<f64> {
    let raw = match call.duration.as_ref()? {
        Value::Number(number) => number.as_f64().or_else(|| number.as_i64().map(|v| v as f64)),
        Value::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    raw.is_finite().then_some(raw)
}

/// Whole seconds, never negative: a negative duration is Apple noise, not a fact to store.
pub fn call_duration_seconds(call: &AppleCallRecord) -> Option<i64> {
    call_duration_raw(call).map(|raw| raw.round().max(0.0) as i64)
}

/// The exported timestamp, trimmed. Apple cannot date a call whose row has no date column.
pub fn call_date_iso(call: &AppleCallRecord) -> Option<String> {
    trimmed(call.date_iso.as_deref())
}

/// FaceTime when either signal says so. This mirrors the deleted TypeScript exactly: the provider
/// string (`com.apple.FaceTime`) and the call type are both consulted, case-insensitively.
pub fn is_facetime_call(call: &AppleCallRecord) -> bool {
    let provider = call.service_provider.as_deref().unwrap_or_default().to_lowercase();
    let call_type = scalar_text(call.call_type.as_ref()).unwrap_or_default().to_lowercase();
    provider.contains("facetime") || call_type.contains("facetime")
}

/// Which relationship source this call belongs to.
pub fn call_source(call: &AppleCallRecord) -> &'static str {
    if is_facetime_call(call) {
        APPLE_FACETIME_SOURCE
    } else {
        APPLE_CALLS_SOURCE
    }
}

/// Direction on the canonical interaction (`outbound` / `inbound`), derived from `originated` only.
pub fn call_direction(call: &AppleCallRecord) -> &'static str {
    match flag(call.originated.as_ref()) {
        Some(true) => "outbound",
        _ => "inbound",
    }
}

/// Direction as the landing table words it (`outgoing` / `incoming`) — the landing vocabulary is
/// the source's, the canonical one is ours, and both are recorded rather than reconciled away.
pub fn call_landing_direction(call: &AppleCallRecord) -> &'static str {
    match flag(call.originated.as_ref()) {
        Some(true) => "outgoing",
        _ => "incoming",
    }
}

// ---------------------------------------------------------------------------
// Evidence: one row per counterparty address, per source.
// ---------------------------------------------------------------------------

/// Build the evidence rows for one call export.
///
/// The evidence row is keyed by the textual address, so every call to that address aggregates into
/// one row carrying the counts and the observation windows — a call is a count, never a transcript.
/// Sources are never merged: the same number reached by phone and by FaceTime is two channels.
pub fn build_call_evidence(
    calls: &[AppleCallRecord],
    source_account: &str,
) -> Vec<AppleHandleEvidence> {
    let mut groups: BTreeMap<(String, String), Vec<&AppleCallRecord>> = BTreeMap::new();
    for call in calls {
        let Some(address) = trimmed(call.address.as_deref()) else {
            continue;
        };
        groups
            .entry((call_source(call).to_owned(), address))
            .or_default()
            .push(call);
    }

    let mut out = Vec::with_capacity(groups.len());
    for ((source, address), rows) in groups {
        let (emails, phones) = handle_to_identities(&address);
        let mut first_observed_at: Option<String> = None;
        let mut last_observed_at: Option<String> = None;
        let mut last_inbound_at: Option<String> = None;
        let mut last_outbound_at: Option<String> = None;
        let mut inbound_count = 0i64;
        let mut outbound_count = 0i64;

        for row in &rows {
            let outbound = call_direction(row) == "outbound";
            if outbound {
                outbound_count += 1;
            } else {
                inbound_count += 1;
            }
            let Some(iso) = call_date_iso(row) else {
                continue;
            };
            if first_observed_at
                .as_deref()
                .map_or(true, |current| iso.as_str() < current)
            {
                first_observed_at = Some(iso.clone());
            }
            if last_observed_at
                .as_deref()
                .map_or(true, |current| iso.as_str() > current)
            {
                last_observed_at = Some(iso.clone());
            }
            if outbound {
                if last_outbound_at
                    .as_deref()
                    .map_or(true, |current| iso.as_str() > current)
                {
                    last_outbound_at = Some(iso);
                }
            } else if last_inbound_at
                .as_deref()
                .map_or(true, |current| iso.as_str() > current)
            {
                last_inbound_at = Some(iso);
            }
        }

        let mut handle_rowids: Vec<i64> = rows.iter().filter_map(|row| row.rowid).collect();
        handle_rowids.sort_unstable();
        handle_rowids.dedup();
        let mut source_ids: Vec<String> = rows.iter().filter_map(|row| call_unique_id(row)).collect();
        source_ids.sort();
        source_ids.dedup();

        // Content-addressed replay key. The deleted TypeScript hashed a JSON object; this is the
        // same facts in the pipe-joined form the rest of the Rust intake uses, so a fingerprint
        // rewritten by this pass still describes the same row — the evidence upsert is an update in
        // place keyed on (source, source_account, source_identity_key), never a second row.
        let evidence_fingerprint = fingerprint(&format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            source,
            source_account,
            address,
            first_observed_at.as_deref().unwrap_or(""),
            last_observed_at.as_deref().unwrap_or(""),
            last_inbound_at.as_deref().unwrap_or(""),
            last_outbound_at.as_deref().unwrap_or(""),
            inbound_count,
            outbound_count,
            source_ids.join(","),
        ));

        let is_two_way = if inbound_count > 0 && outbound_count > 0 {
            Some(true)
        } else if !rows.is_empty() {
            Some(false)
        } else {
            None
        };

        out.push(AppleHandleEvidence {
            source: source.to_owned(),
            source_account: source_account.to_owned(),
            source_identity_key: address,
            source_label: Some(
                if source == APPLE_FACETIME_SOURCE {
                    "FaceTime"
                } else {
                    "Phone"
                }
                .to_owned(),
            ),
            display_name: None,
            organization: None,
            has_email: !emails.is_empty(),
            has_phone: !phones.is_empty(),
            emails,
            phones,
            first_observed_at,
            last_observed_at,
            last_inbound_at,
            last_outbound_at,
            inbound_count,
            outbound_count,
            is_two_way,
            is_owner_initiated: (outbound_count > 0).then_some(true),
            is_automated_or_bulk: None,
            is_organization_or_service: None,
            known_apple_contact: Some(false),
            coverage_note: Some("Apple CallHistoryDB read-only export".to_owned()),
            evidence_fingerprint,
            handle_rowids,
        });
    }
    out
}


// ---------------------------------------------------------------------------
// The canonical interaction a call becomes.
// ---------------------------------------------------------------------------

/// The row `interaction` receives for one Person x call channel: the NEWEST call, not one row per
/// call. ODS keeps every call; the client pane shows one Phone line and one FaceTime line.
#[derive(Debug, Clone)]
pub struct AppleCallInteraction {
    pub person_id: String,
    pub channel: &'static str,
    pub event_type: &'static str,
    pub direction: &'static str,
    pub occurred_at: String,
    /// Whole seconds, rounded here (never at landing) — the `interaction.duration_seconds` column is
    /// an integer.
    pub duration_seconds: Option<i64>,
    pub source_system: &'static str,
    pub source_external_id: String,
    pub source_metadata: Value,
}

/// A call only becomes an interaction when it has a counterparty address, a date, and a canonical
/// person. Without a person there is nothing to remember it against — the evidence row is where
/// that call stays until reconciliation links it.
pub fn call_latest_interaction(
    call: &AppleCallRecord,
    person_id: &str,
) -> Option<AppleCallInteraction> {
    let address = trimmed(call.address.as_deref())?;
    let occurred_at = call_date_iso(call)?;
    let source = call_source(call);
    Some(AppleCallInteraction {
        person_id: person_id.to_owned(),
        channel: "call",
        event_type: if source == APPLE_FACETIME_SOURCE {
            "facetime_call"
        } else {
            "phone_call"
        },
        direction: call_direction(call),
        occurred_at,
        duration_seconds: call_duration_seconds(call),
        source_system: source,
        // Keyed on the source, never on the call: each run UPDATES that one row instead of
        // appending another, and an older call can never overwrite a newer one.
        source_external_id: format!("latest:{person_id}:call"),
        source_metadata: json!({
            "address": address,
            "answered": flag(call.answered.as_ref()),
            "callType": scalar_text(call.call_type.as_ref()),
            "serviceProvider": call.service_provider,
            "countryCode": call.country_code,
        }),
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn call(
        unique_id: &str,
        address: &str,
        date: &str,
        originated: i64,
        provider: Option<&str>,
    ) -> AppleCallRecord {
        AppleCallRecord {
            rowid: Some(1),
            unique_id: Some(unique_id.to_owned()),
            address: Some(address.to_owned()),
            date_iso: Some(date.to_owned()),
            originated: Some(json!(originated)),
            service_provider: provider.map(str::to_owned),
            call_type: Some(json!(1)),
            duration: Some(json!(42.4)),
            ..Default::default()
        }
    }

    /// Two calls to one number (one placed, one received) and a FaceTime call to an email. The
    /// evidence row is keyed by the textual address, exactly like the source export, and each call
    /// keeps its own rowid so the evidence row can be traced back to the calls it counted.
    fn calls() -> Vec<AppleCallRecord> {
        let mut records = vec![
            call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None),
            call("c2", "7875551234", "2026-01-02T10:00:00.000Z", 0, None),
            call(
                "c3",
                "dana@example.com",
                "2026-01-03T10:00:00.000Z",
                0,
                Some("com.apple.FaceTime"),
            ),
        ];
        for (index, record) in records.iter_mut().enumerate() {
            record.rowid = Some(index as i64 + 1);
        }
        records
    }

    #[test]
    fn the_exporters_jsonl_keys_deserialize_as_written() {
        // Copied from a real `calls.jsonl` line: the exporter spells the date `dateISO`, and a
        // `rename_all = "camelCase"` guess silently turned every date into null.
        let line = r#"{"address":"+17875551234","answered":1,"callType":1,"countryCode":"PR",
            "dateISO":"2026-01-03T10:00:00.000Z","dateRaw":758000000,"duration":606.3955090045929,
            "originated":1,"rowid":42,"serviceProvider":"com.apple.FaceTime","uniqueId":"ABC-1"}"#;
        let call: AppleCallRecord = serde_json::from_str(line).expect("a real export line parses");
        assert_eq!(call.unique_id.as_deref(), Some("ABC-1"));
        assert_eq!(call.date_iso.as_deref(), Some("2026-01-03T10:00:00.000Z"));
        assert_eq!(call_date_iso(&call).as_deref(), Some("2026-01-03T10:00:00.000Z"));
        assert_eq!(call.rowid, Some(42));
        assert_eq!(flag(call.originated.as_ref()), Some(true));
        assert_eq!(flag(call.answered.as_ref()), Some(true));
        assert!(is_facetime_call(&call));
        assert_eq!(call_duration_raw(&call), Some(606.3955090045929));
        assert_eq!(
            call_duration_seconds(&call),
            Some(606),
            "the CRM rounds; the landing row keeps Apple's precision"
        );
    }

    #[test]
    fn facetime_is_decided_by_provider_or_call_type_never_guessed() {
        let mut plain = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None);
        assert!(!is_facetime_call(&plain));
        assert_eq!(call_source(&plain), APPLE_CALLS_SOURCE);

        plain.service_provider = Some("com.apple.FaceTime".to_owned());
        assert!(is_facetime_call(&plain));
        assert_eq!(call_source(&plain), APPLE_FACETIME_SOURCE);

        plain.service_provider = None;
        plain.call_type = Some(json!("FaceTime"));
        assert!(
            is_facetime_call(&plain),
            "the call type is the second signal, case-insensitively"
        );

        plain.call_type = Some(json!("7"));
        assert!(!is_facetime_call(&plain));
    }

    #[test]
    fn direction_and_replay_identity_survive_apple_types() {
        let placed = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None);
        assert_eq!(call_direction(&placed), "outbound");
        assert_eq!(call_landing_direction(&placed), "outgoing");

        let mut received = placed.clone();
        received.originated = Some(json!(0));
        assert_eq!(call_direction(&received), "inbound");
        assert_eq!(call_landing_direction(&received), "incoming");

        received.originated = Some(json!(true));
        assert_eq!(call_direction(&received), "outbound");

        received.unique_id = Some("  ".to_owned());
        received.rowid = Some(42);
        assert_eq!(
            call_unique_id(&received).as_deref(),
            Some("42"),
            "an absent unique id falls back to the exporter's rowid"
        );

        let mut duration = placed.clone();
        duration.duration = Some(json!(-3.2));
        assert_eq!(call_duration_seconds(&duration), Some(0));
        duration.duration = Some(json!("12.6"));
        assert_eq!(call_duration_seconds(&duration), Some(13));
        duration.duration = Some(Value::Null);
        assert_eq!(call_duration_seconds(&duration), None);
    }

    #[test]
    fn evidence_aggregates_by_address_and_keeps_channels_apart() {
        let evidence = build_call_evidence(&calls(), APPLE_CALL_HISTORY_ACCOUNT);
        assert_eq!(evidence.len(), 2, "one phone identity and one FaceTime identity");

        let phone = evidence
            .iter()
            .find(|row| row.source_identity_key == "7875551234")
            .expect("phone evidence");
        assert_eq!(phone.source, APPLE_CALLS_SOURCE);
        assert_eq!(phone.source_label.as_deref(), Some("Phone"));
        assert_eq!(phone.inbound_count, 1);
        assert_eq!(phone.outbound_count, 1);
        assert_eq!(phone.is_two_way, Some(true));
        assert_eq!(phone.is_owner_initiated, Some(true));
        assert!(phone.has_phone);
        assert!(!phone.has_email);
        assert_eq!(phone.handle_rowids, vec![1, 2]);
        assert_eq!(phone.evidence_fingerprint.len(), 16);

        let facetime = evidence
            .iter()
            .find(|row| row.source == APPLE_FACETIME_SOURCE)
            .expect("facetime evidence");
        assert_eq!(facetime.source_identity_key, "dana@example.com");
        assert_eq!(facetime.source_label.as_deref(), Some("FaceTime"));
        assert!(facetime.has_email);
        assert_eq!(facetime.inbound_count, 1);
        assert_eq!(facetime.outbound_count, 0);
        assert_eq!(facetime.is_two_way, Some(false));
        assert_eq!(facetime.is_owner_initiated, None);
        assert_eq!(facetime.handle_rowids, vec![3]);
        assert_eq!(
            facetime.coverage_note.as_deref(),
            Some("Apple CallHistoryDB read-only export")
        );
    }

    #[test]
    fn the_same_export_always_hashes_the_same() {
        let first = build_call_evidence(&calls(), APPLE_CALL_HISTORY_ACCOUNT);
        let second = build_call_evidence(&calls(), APPLE_CALL_HISTORY_ACCOUNT);
        for index in 0..first.len() {
            assert_eq!(
                first[index].evidence_fingerprint, second[index].evidence_fingerprint,
                "a replay of the same export is the same fingerprint"
            );
        }
    }

    #[test]
    fn a_call_becomes_the_interaction_the_client_pane_reads() {
        let records = calls();
        let interaction =
            call_latest_interaction(&records[0], "00000000-0000-0000-0000-000000000001")
                .expect("placed call becomes an interaction");
        assert_eq!(interaction.channel, "call");
        assert_eq!(interaction.event_type, "phone_call");
        assert_eq!(interaction.direction, "outbound");
        assert_eq!(interaction.source_system, APPLE_CALLS_SOURCE);
        assert_eq!(
            interaction.source_external_id,
            "latest:00000000-0000-0000-0000-000000000001:call"
        );
        assert_eq!(interaction.source_metadata["address"], json!("7875551234"));
        assert_eq!(interaction.source_metadata["answered"], Value::Null);
        assert_eq!(
            interaction.duration_seconds,
            Some(42),
            "Apple's 42.4s is rounded for the CRM, not at landing"
        );

        let interaction =
            call_latest_interaction(&records[2], "00000000-0000-0000-0000-000000000002")
                .expect("facetime call becomes an interaction");
        assert_eq!(interaction.event_type, "facetime_call");
        assert_eq!(interaction.direction, "inbound");
        assert_eq!(interaction.source_system, APPLE_FACETIME_SOURCE);

        let mut undated = records[0].clone();
        undated.date_iso = None;
        assert!(
            call_latest_interaction(&undated, "00000000-0000-0000-0000-000000000001").is_none(),
            "a call with no date is not a moment to remember"
        );
    }
}


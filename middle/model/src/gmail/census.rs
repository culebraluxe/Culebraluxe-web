// ---------------------------------------------------------------------------
// Gmail relationship census — the Rust replacement for
// `lib/relationship-intel/gmail-census.ts`, deleted with the TypeScript engine in the 2026-09 port
// (commit 4cf98110) and recovered from that commit. Same artifact shape, same column semantics, same
// batch accounting: a translation, not a re-invention.
//
// One row per normalized external email correspondent, parsed into the source-neutral evidence shape
// the reconciliation already understands. This is a BOUNDED batch, not a full-mailbox census: the
// artifact's own coverage bounds travel in `coverage_note` and in the accounting below.
//
// The artifact carries aggregate correspondent evidence only — no message bodies, attachments or
// snippets — and a row that cannot be read is refused WITH A REASON rather than silently dropped.
// The batch balances, and a duplicate is named rather than quietly merged:
//
//   declared = accepted + rejected + quarantined + deduplicated
//
// Nothing here decides who anyone is. The evidence goes to the crate's one adjudicator
// (`decide_apple_handle`) and the caller writes that decision, so the census can never become a
// second voice about identity.
// ---------------------------------------------------------------------------
use super::GMAIL_CONTEXT_SOURCE;
use crate::apple_messages::{fingerprint, normalize_email, AppleHandleEvidence, IdentityEvidence};
use std::collections::HashSet;

/// One accepted row: neutral evidence, keyed `(source, source_account, source_identity_key)`.
#[derive(Debug, Clone)]
pub struct GmailCensusRow {
    pub evidence: AppleHandleEvidence,
}

/// A row that was refused, with the 1-based line of the artifact it came from and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailCensusRefusal {
    pub line: usize,
    pub reason: &'static str,
}

/// The whole bounded batch, with the accounting that makes it auditable.
#[derive(Debug, Clone, Default)]
pub struct GmailCensusBatch {
    pub declared: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub quarantined: usize,
    pub deduplicated: usize,
    pub rows: Vec<GmailCensusRow>,
    pub rejections: Vec<GmailCensusRefusal>,
    pub quarantine: Vec<GmailCensusRefusal>,
}

impl GmailCensusBatch {
    /// `declared = accepted + rejected + quarantined + deduplicated`. A batch that does not balance
    /// has lost a row, and a load reporting success over a lost row is the failure this accounting
    /// exists to catch.
    ///
    /// The deleted parser wrote this invariant without the `deduplicated` bucket, so an artifact
    /// carrying one repeated correspondent failed its own check. All four buckets are named here: a
    /// duplicate is accounted for, not pretended away.
    pub fn balances(&self) -> bool {
        self.declared == self.accepted + self.rejected + self.quarantined + self.deduplicated
    }
}

/// The reasons that make a row *rejected*: the identity itself is unusable. Every other reason is a
/// *quarantine* — a row that is readable but not safely interpretable, held for review rather than
/// thrown away.
///
/// These three are every reason this parser can raise, so today the quarantine bucket is always
/// empty; it stays in the accounting because it is the fail-safe for a reason a future artifact
/// teaches us (`unknown reason ⇒ quarantine`, never a silent accept). The deleted parser listed a
/// fourth name, `missing_email`, which `normalize_email` cannot return — dead code, not a rule.
const REJECT_REASONS: [&str; 3] = ["empty", "too_long", "invalid_format"];

/// Minimal CSV line parser that honours double-quoted fields (display-name candidates contain
/// commas, and a quoted field may carry a doubled quote for a literal one).
pub fn parse_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            out.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    out.push(current);
    out
}

/// Why a value is not a usable email — the names the deleted parser published: `normalizeEmail`
/// refused for exactly one of these, and the reason decided reject versus quarantine.
fn email_refusal(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "empty"
    } else if trimmed.encode_utf16().count() > 320 {
        "too_long"
    } else {
        "invalid_format"
    }
}

/// A count column. Non-numeric or absent counts as zero observed messages.
///
/// Deviation from the deleted parser, stated because a fingerprint depends on it: that parser could
/// carry `null` in a count column, while the canonical evidence field is `i64`, so an unreadable
/// count is recorded as `0` and fingerprinted as `0`. A row stored before the port with `null` there
/// therefore reads as a **changed payload** on replay rather than an exact replay — which is the
/// honest answer, not a fingerprint that describes a value nobody stored.
fn to_int(value: Option<&str>) -> i64 {
    value
        .map(str::trim)
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .map(|number| number as i64)
        .unwrap_or(0)
}

/// A boolean column, or `None` when the artifact did not say. `true`/`1` and `false`/`0` only: an
/// unrecognized token is not quietly read as `false`.
fn to_bool(value: Option<&str>) -> Option<bool> {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.eq_ignore_ascii_case("true") || trimmed == "1" {
        Some(true)
    } else if trimmed.eq_ignore_ascii_case("false") || trimmed == "0" {
        Some(false)
    } else {
        None
    }
}

/// Spreadsheet-formula injection guard: a leading `=` `+` `-` `@` is neutralized, so a display name
/// carried out of the artifact can never execute as a formula on the way through.
fn sanitize_spreadsheet_cell(value: &str) -> String {
    let trimmed = value.trim();
    match trimmed.chars().next() {
        Some(first) if matches!(first, '=' | '+' | '-' | '@') => format!("'{trimmed}"),
        _ => trimmed.to_owned(),
    }
}

/// One JSON value the way the deleted parser's `JSON.stringify` rendered it (compact; `null` for
/// absent), so a row loaded before the port keeps the fingerprint it already had.
fn json_string(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "null".to_owned();
    };
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The fingerprinted payload of one row: the same fields, in the same order, as the deleted parser,
/// so an exact replay is still a replay rather than a changed payload.
fn fingerprint_input(evidence: &AppleHandleEvidence) -> String {
    format!(
        concat!(
            "{{\"source\":{},\"sourceAccount\":{},\"sourceIdentityKey\":{},\"displayName\":{},",
            "\"lastInboundAt\":{},\"lastOutboundAt\":{},\"inboundCount\":{},\"outboundCount\":{},",
            "\"isTwoWay\":{},\"isAutomatedOrBulk\":{},\"coverageNote\":{}}}"
        ),
        json_string(Some(evidence.source.as_str())),
        json_string(Some(evidence.source_account.as_str())),
        json_string(Some(evidence.source_identity_key.as_str())),
        json_string(evidence.display_name.as_deref()),
        json_string(evidence.last_inbound_at.as_deref()),
        json_string(evidence.last_outbound_at.as_deref()),
        evidence.inbound_count,
        evidence.outbound_count,
        match evidence.is_two_way {
            Some(value) => value.to_string(),
            None => "null".to_owned(),
        },
        match evidence.is_automated_or_bulk {
            Some(value) => value.to_string(),
            None => "null".to_owned(),
        },
        json_string(evidence.coverage_note.as_deref()),
    )
}

/// One CSV row into neutral evidence, or the reason it cannot be read.
///
/// Column semantics are the artifact's and unchanged: `0` the normalized email, `1` display-name
/// candidates, `3` the opaque account token (never a credential), `4`/`5` first and last observed,
/// `7`/`9` last inbound and outbound, `10`/`11` the inbound and outbound message counts, `18`
/// two-way, `19` owner-initiated, `20` or `21` automated-or-bulk, `22` organization-or-service, `28`
/// the coverage limitation the artifact declares about itself.
pub fn parse_gmail_census_row(raw: &str) -> Result<GmailCensusRow, &'static str> {
    let cols = parse_csv_line(raw);
    let column = |index: usize| cols.get(index).map(String::as_str).unwrap_or_default();
    let text = |index: usize| {
        let value = column(index).trim();
        (!value.is_empty()).then(|| value.to_owned())
    };

    let email_value = column(0).trim();
    let Some(email_normalized) = normalize_email(email_value) else {
        return Err(email_refusal(email_value));
    };

    let display_name = {
        let value = sanitize_spreadsheet_cell(column(1));
        (!value.is_empty()).then_some(value)
    };
    let source_account = {
        let value = sanitize_spreadsheet_cell(column(3));
        if value.is_empty() {
            "unknown".to_owned()
        } else {
            value
        }
    };
    let coverage_note = {
        let value = column(27).trim();
        (!value.is_empty()).then(|| value.to_owned())
    };

    let mut evidence = AppleHandleEvidence {
        source: GMAIL_CONTEXT_SOURCE.to_owned(),
        source_account,
        source_identity_key: email_normalized.clone(),
        source_label: None,
        display_name,
        organization: None,
        emails: vec![IdentityEvidence {
            value: email_value.to_owned(),
            normalized: email_normalized,
            label: None,
        }],
        phones: Vec::new(),
        first_observed_at: text(4),
        last_observed_at: text(5),
        last_inbound_at: text(7),
        last_outbound_at: text(9),
        inbound_count: to_int(Some(column(10))),
        outbound_count: to_int(Some(column(11))),
        is_two_way: to_bool(Some(column(18))),
        is_owner_initiated: to_bool(Some(column(19))),
        is_automated_or_bulk: Some(
            to_bool(Some(column(20)))
                .or_else(|| to_bool(Some(column(21))))
                .unwrap_or(false),
        ),
        is_organization_or_service: to_bool(Some(column(22))),
        known_apple_contact: Some(false),
        has_email: true,
        has_phone: false,
        coverage_note,
        evidence_fingerprint: String::new(),
        handle_rowids: Vec::new(),
    };
    evidence.evidence_fingerprint = fingerprint(&fingerprint_input(&evidence));
    Ok(GmailCensusRow { evidence })
}

/// A whole census artifact into accepted rows plus the accounting of everything else.
///
/// The header row is dropped only when the first non-blank line announces itself as the column list
/// this parser expects; line numbers are the artifact's real ones, so a refusal names the line an
/// operator can open. Deduplication is by `(source_account, source_identity_key)` and the first
/// occurrence wins — the same rule as the evidence unique key, so a batch cannot carry two rows that
/// would fight over one evidence row.
pub fn parse_gmail_census(csv: &str) -> GmailCensusBatch {
    let mut batch = GmailCensusBatch::default();
    let mut body: Vec<(usize, &str)> = csv
        .lines()
        .enumerate()
        .filter(|(_, raw)| !raw.trim().is_empty())
        .map(|(index, raw)| (index + 1, raw))
        .collect();
    if body.first().is_some_and(|(_, raw)| {
        raw.trim_start()
            .to_lowercase()
            .starts_with("normalized_email")
    }) {
        body.remove(0);
    }

    batch.declared = body.len();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for (line, raw) in body {
        match parse_gmail_census_row(raw) {
            Ok(row) => {
                let key = (
                    row.evidence.source_account.clone(),
                    row.evidence.source_identity_key.clone(),
                );
                if !seen.insert(key) {
                    batch.deduplicated += 1;
                    continue;
                }
                batch.accepted += 1;
                batch.rows.push(row);
            }
            Err(reason) => {
                let refusal = GmailCensusRefusal { line, reason };
                if REJECT_REASONS.contains(&reason) {
                    batch.rejected += 1;
                    batch.rejections.push(refusal);
                } else {
                    batch.quarantined += 1;
                    batch.quarantine.push(refusal);
                }
            }
        }
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 28-column synthetic row in the artifact's own column order, with only the columns this
    /// parser reads filled in. Synthetic on purpose: the real census is a private artifact about real
    /// people, and a test must not carry one into the tree.
    fn synthetic_row(email: &str) -> String {
        let mut cells = vec![String::new(); 28];
        for (index, value) in [
            (0, email),
            (1, "Dana Example"),
            (2, "gmail"),
            (3, "acct-token"),
            (4, "2024-01-01T00:00:00.000Z"),
            (5, "2026-08-01T12:00:00.000Z"),
            (6, "2024-01-01T00:00:00.000Z"),
            (7, "2026-08-01T11:00:00.000Z"),
            (8, "2024-01-02T00:00:00.000Z"),
            (9, "2026-07-30T09:30:00.000Z"),
            (10, "12"),
            (11, "4"),
            (18, "true"),
            (19, "false"),
            (20, "false"),
            (21, "false"),
            (22, "false"),
            // A quoted field the way the artifact writes any value containing a comma.
            (27, "\"bounded window: newest 5,000 threads\""),
        ] {
            cells[index] = value.to_owned();
        }
        cells.join(",")
    }

    const HEADER: &str = "normalized_email,display_name_candidates";

    #[test]
    fn a_row_becomes_neutral_evidence_for_the_gmail_source() {
        let row = parse_gmail_census_row(&synthetic_row("Dana@Example.COM")).expect("parses");
        let evidence = row.evidence;
        assert_eq!(evidence.source, GMAIL_CONTEXT_SOURCE);
        assert_eq!(evidence.source_account, "acct-token");
        assert_eq!(evidence.source_identity_key, "dana@example.com");
        assert_eq!(evidence.display_name.as_deref(), Some("Dana Example"));
        assert_eq!(evidence.emails.len(), 1);
        assert_eq!(evidence.emails[0].value, "Dana@Example.COM");
        assert_eq!(evidence.emails[0].normalized, "dana@example.com");
        assert_eq!(
            evidence.last_inbound_at.as_deref(),
            Some("2026-08-01T11:00:00.000Z")
        );
        assert_eq!(
            evidence.last_outbound_at.as_deref(),
            Some("2026-07-30T09:30:00.000Z")
        );
        assert_eq!(evidence.inbound_count, 12);
        assert_eq!(evidence.outbound_count, 4);
        assert_eq!(evidence.is_two_way, Some(true));
        assert_eq!(evidence.is_owner_initiated, Some(false));
        assert_eq!(evidence.is_automated_or_bulk, Some(false));
        assert_eq!(evidence.is_organization_or_service, Some(false));
        assert_eq!(
            evidence.coverage_note.as_deref(),
            Some("bounded window: newest 5,000 threads"),
            "the artifact's own coverage limitation is preserved, not dropped"
        );
        // The census reads correspondence, not address books: no handle rowids, and the fact that
        // Gmail did not know this person is recorded rather than assumed.
        assert_eq!(evidence.known_apple_contact, Some(false));
        assert!(evidence.handle_rowids.is_empty());
        assert!(evidence.phones.is_empty());
        assert!(evidence.has_email && !evidence.has_phone);
        assert!(!evidence.evidence_fingerprint.is_empty());
    }

    /// The fingerprint payload is what makes a replay recognizable, so its field set, order and
    /// rendering are pinned against a literal rather than derived from the code under test.
    #[test]
    fn the_fingerprint_payload_keeps_the_deleted_parsers_shape() {
        let row = parse_gmail_census_row(&synthetic_row("dana@example.com")).expect("parses");
        let expected = concat!(
            r#"{"source":"gmail_contacts","sourceAccount":"acct-token","#,
            r#""sourceIdentityKey":"dana@example.com","displayName":"Dana Example","#,
            r#""lastInboundAt":"2026-08-01T11:00:00.000Z","#,
            r#""lastOutboundAt":"2026-07-30T09:30:00.000Z","inboundCount":12,"outboundCount":4,"#,
            r#""isTwoWay":true,"isAutomatedOrBulk":false,"#,
            r#""coverageNote":"bounded window: newest 5,000 threads"}"#,
        );
        assert_eq!(fingerprint_input(&row.evidence), expected);
        assert_eq!(row.evidence.evidence_fingerprint, fingerprint(expected));
    }

    #[test]
    fn a_boolean_the_artifact_did_not_state_is_not_read_as_false() {
        let mut cells = parse_csv_line(&synthetic_row("dana@example.com"));
        cells[18] = String::new();
        cells[19] = "maybe".to_owned();
        let row = parse_gmail_census_row(&cells.join(",")).expect("parses");
        assert_eq!(
            row.evidence.is_two_way, None,
            "an empty cell is not `false`"
        );
        assert_eq!(
            row.evidence.is_owner_initiated, None,
            "an unrecognized token is not `false` either"
        );
    }

    #[test]
    fn the_automated_flag_falls_back_to_the_second_column_then_to_false() {
        let mut cells = parse_csv_line(&synthetic_row("dana@example.com"));
        cells[20] = String::new();
        cells[21] = "true".to_owned();
        let row = parse_gmail_census_row(&cells.join(",")).expect("parses");
        assert_eq!(row.evidence.is_automated_or_bulk, Some(true));

        cells[20] = String::new();
        cells[21] = String::new();
        let row = parse_gmail_census_row(&cells.join(",")).expect("parses");
        assert_eq!(
            row.evidence.is_automated_or_bulk,
            Some(false),
            "absence of a bulk signal is `false`, never `None`: the column is a decision, not a gap"
        );
    }

    #[test]
    fn a_batch_balances_and_names_the_row_it_deduplicated() {
        let duplicate = synthetic_row("dana@example.com");
        let second = synthetic_row("eric@example.org");
        let batch =
            parse_gmail_census(&format!("{HEADER}\n{duplicate}\n{second}\n\n{duplicate}\n"));
        assert_eq!(batch.declared, 3, "blank lines are not rows");
        assert_eq!(batch.accepted, 2);
        assert_eq!(batch.rejected, 0);
        assert_eq!(batch.quarantined, 0);
        assert_eq!(
            batch.deduplicated, 1,
            "a repeat is accounted for, never silently merged"
        );
        assert!(
            batch.balances(),
            "every declared row is in exactly one bucket"
        );
        assert_eq!(batch.rows.len(), 2);
        assert_eq!(
            batch.rows[0].evidence.source_identity_key,
            "dana@example.com"
        );
        assert_eq!(
            batch.rows[1].evidence.source_identity_key,
            "eric@example.org"
        );
    }

    #[test]
    fn a_batch_without_a_header_row_is_still_a_batch() {
        let batch = parse_gmail_census(&format!("{}\n", synthetic_row("dana@example.com")));
        assert_eq!(batch.declared, 1);
        assert_eq!(batch.accepted, 1);
        assert!(batch.balances());
    }

    #[test]
    fn an_unusable_identity_is_refused_with_the_line_and_the_reason() {
        let long_domain = format!("{}@example.com", "a".repeat(310));
        let batch = parse_gmail_census(&format!(
            "{HEADER}\n{}\n,\nnot an email\n{long_domain}\n",
            synthetic_row("dana@example.com"),
        ));
        assert_eq!(batch.declared, 4);
        assert_eq!(batch.accepted, 1);
        assert_eq!(batch.rejected, 3);
        assert!(batch.balances());
        assert_eq!(
            batch.rejections,
            vec![
                GmailCensusRefusal {
                    line: 3,
                    reason: "empty"
                },
                GmailCensusRefusal {
                    line: 4,
                    reason: "invalid_format"
                },
                GmailCensusRefusal {
                    line: 5,
                    reason: "too_long"
                },
            ],
            "a refusal names the artifact line an operator can open, in order"
        );
    }

    /// Every reason this parser can raise is a rejection: a row it cannot read has no identity to
    /// reconcile. Pinned so the quarantine bucket cannot start absorbing malformed rows unnoticed.
    #[test]
    fn every_reachable_refusal_is_a_rejection() {
        let batch = parse_gmail_census(&format!("{HEADER}\n,\nnot an email\n"));
        assert_eq!(batch.rejected, 2);
        assert_eq!(batch.quarantined, 0);
        assert!(batch.quarantine.is_empty());
        assert!(batch.balances());
    }

    #[test]
    fn a_quoted_field_survives_commas_and_doubled_quotes() {
        let cells = parse_csv_line("\"a, b\",\"he said \"\"hello\"\"\",plain");
        assert_eq!(cells, vec!["a, b", "he said \"hello\"", "plain"]);
    }

    #[test]
    fn a_carried_value_cannot_carry_a_spreadsheet_formula() {
        let mut cells = parse_csv_line(&synthetic_row("dana@example.com"));
        cells[1] = "\"=1+1\"".to_owned();
        let row = parse_gmail_census_row(&cells.join(",")).expect("parses");
        assert_eq!(row.evidence.display_name.as_deref(), Some("'=1+1"));
    }

    #[test]
    fn a_missing_account_token_is_unknown_and_never_a_credential() {
        let mut cells = parse_csv_line(&synthetic_row("dana@example.com"));
        cells[3] = String::new();
        let row = parse_gmail_census_row(&cells.join(",")).expect("parses");
        assert_eq!(row.evidence.source_account, "unknown");
        assert_eq!(row.evidence.source_identity_key, "dana@example.com");
    }
}

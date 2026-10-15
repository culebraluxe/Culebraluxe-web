// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use chrono::Utc;

fn message(payload: serde_json::Value) -> EmailMessage {
    let now = Utc::now().to_rfc3339();
    EmailMessage {
        id: "11111111-1111-4111-8111-111111111111".into(),
        message_kind: EmailMessageKind::SignatureInvitation,
        recipient_email: "buyer@example.test".into(),
        template_key: "luxesign.invitation".into(),
        template_payload: payload,
        dedupe_key: "invite-1".into(),
        status: EmailMessageStatus::Queued,
        provider_message_id: None,
        attempt_count: 0,
        last_error: None,
        correlation_id: Some("corr-1".into()),
        causation_id: None,
        queued_at: now.clone(),
        sent_at: None,
        updated_at: now,
    }
}

#[test]
fn invitation_renderer_requires_the_recipient_signing_url() {
    let error = render_email(&message(serde_json::json!({
        "recipientName": "Buyer"
    })))
    .unwrap_err();
    assert_eq!(error.code(), "EMAIL_TEMPLATE_INVALID");

    let mail = render_email(&message(serde_json::json!({
        "recipientName": "Buyer",
        "signingUrl": "https://example.test/sign/token"
    })))
    .unwrap();
    assert_eq!(mail.to, "buyer@example.test");
    assert!(mail.text.contains("https://example.test/sign/token"));
    assert!(mail.text.contains("Do not forward"));
    let html = mail.html.expect("the invitation carries an HTML part");
    assert!(html.contains(r#"href="https://example.test/sign/token""#));
    assert!(html.contains("Review and sign"));
}

#[test]
fn the_html_part_escapes_what_a_person_typed_and_refuses_a_non_http_link() {
    let mail = render_email(&message(serde_json::json!({
        "recipientName": "<script>alert(1)</script>",
        "signingUrl": "https://example.test/sign/a\"onmouseover=\"x",
        "message": "Fish & <b>chips</b>"
    })))
    .unwrap();
    let html = mail.html.unwrap();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("Fish &amp; &lt;b&gt;chips&lt;/b&gt;"));
    assert!(!html.contains(r#"a"onmouseover"#));

    let bad = render_email(&message(serde_json::json!({
        "recipientName": "Buyer",
        "signingUrl": "javascript:alert(1)"
    })))
    .unwrap();
    assert!(!bad.html.unwrap().contains("javascript:"));
}

#[test]
fn a_reminder_says_so() {
    let mut reminder = message(serde_json::json!({
        "recipientName": "Buyer",
        "signingUrl": "https://example.test/sign/token"
    }));
    reminder.message_kind = EmailMessageKind::SignatureReminder;
    let mail = render_email(&reminder).unwrap();
    assert!(mail.text.contains("still waiting"));
}

fn outcome(kind: EmailMessageKind, payload: serde_json::Value) -> OutgoingMail {
    let mut message = message(payload);
    message.message_kind = kind;
    render_email(&message).unwrap()
}

#[test]
fn the_completed_notice_names_the_document_and_the_people_and_promises_the_files() {
    let mail = outcome(
        EmailMessageKind::SignatureCompleted,
        serde_json::json!({
            "recipientName": "Maria",
            "documentTitle": "Listing Agreement",
            "signers": ["Maria Alvarez", "Pedro Diaz"],
            "attachCompletion": true,
            "signatureRequestId": "req-1"
        }),
    );
    assert_eq!(mail.subject, "Signed: Listing Agreement — Luxesign");
    assert!(mail
        .text
        .contains("Listing Agreement has been signed by everyone"));
    assert!(mail.text.contains("Maria Alvarez, Pedro Diaz"));
    assert!(mail.text.contains("attached"));
    let html = mail.html.expect("an HTML part");
    assert!(html.contains("Dear Maria,"));
    assert!(
        !html.contains("Review and sign"),
        "a completed notice has no signing button"
    );
}

#[test]
fn a_copy_recipient_without_a_name_is_greeted_plainly() {
    let mail = outcome(
        EmailMessageKind::SignatureCompleted,
        serde_json::json!({ "recipientName": "", "documentTitle": "Deed" }),
    );
    assert!(mail.text.starts_with("Dear Client,\n"));
    assert!(mail
        .text
        .contains("Your CulebraLuxe contact has the signed document"));
}

#[test]
fn the_declined_notice_says_who_and_why_and_escapes_what_a_person_typed() {
    let mail = outcome(
        EmailMessageKind::SignatureDeclined,
        serde_json::json!({
            "recipientName": "Pedro",
            "documentTitle": "Listing Agreement",
            "declinerName": "Maria <b>Alvarez</b>",
            "reason": "Price & terms <script>x</script>"
        }),
    );
    assert_eq!(mail.subject, "Declined: Listing Agreement — Luxesign");
    assert!(mail.text.contains("declined to sign Listing Agreement"));
    assert!(mail.text.contains("Price & terms"));
    let html = mail.html.unwrap();
    assert!(!html.contains("<script>") && !html.contains("<b>Alvarez"));
    assert!(html.contains("Price &amp; terms &lt;script&gt;"));
}

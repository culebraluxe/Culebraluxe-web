// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

#[test]
fn desk_reads_live_and_selects_detail() {
    let (mut model, cmd) = LuxesignSigning::init(&ScreenCtx::default());
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/api/portal/rust-ui/signing/dispatch");

    let rows = vec![crate::model::SigningEnvelopeSummary {
        signature_request_id: "req-1".into(),
        transaction_document_id: "doc-1".into(),
        subject: Some("Listing Contract".into()),
        client_name: Some("Ada".into()),
        signing_mode: "sequential".into(),
        status: "sent".into(),
        issued_at: None,
        expires_at: None,
        recipient_total: 2,
        completed_total: 1,
    }];
    let cmd = LuxesignSigning::update(&mut model, Msg::ListLoaded(Ok(rows)), &ScreenCtx::default());
    // First row auto-selects and reads its detail.
    assert_eq!(model.selected_id.as_deref(), Some("req-1"));
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/api/portal/rust-ui/signing/dispatch");

    // Actions address the durable dispatcher with verified ids.
    model.seq = 7;
    let cmd = LuxesignSigning::update(&mut model, Msg::VoidEnvelope, &ScreenCtx::default());
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/api/portal/rust-ui/signing/command");
}

#[test]
fn import_sends_the_envelope_without_geometry() {
    let (mut model, _) = LuxesignSigning::init(&ScreenCtx::default());
    model.selected_id = Some("req-9".into());
    model.seq = 3;
    let cmd = LuxesignSigning::update(&mut model, Msg::ImportFields, &ScreenCtx::default());
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/api/portal/rust-ui/signing/command");
}

#[test]
fn sending_is_one_command_and_closes_the_panel_when_it_succeeds() {
    let ctx = ScreenCtx::default();
    let (mut model, _) = LuxesignSigning::init(&ctx);
    LuxesignSigning::update(&mut model, Msg::OpenCompose, &ctx);
    for edit in [
        ComposeEdit::Document("doc-1".into()),
        ComposeEdit::Name(0, "Ada".into()),
        ComposeEdit::Email(0, "ada@example.com".into()),
    ] {
        LuxesignSigning::update(&mut model, Msg::Compose(edit), &ctx);
    }
    // An incomplete draft is refused without a request.
    let mut bad = model.clone();
    LuxesignSigning::update(
        &mut bad,
        Msg::Compose(ComposeEdit::Email(0, "nope".into())),
        &ctx,
    );
    let cmd = LuxesignSigning::update(&mut bad, Msg::SendCompose, &ctx);
    assert!(cmd.into_requests().is_empty());
    assert!(bad.compose.as_ref().unwrap().error.is_some());

    let cmd = LuxesignSigning::update(&mut model, Msg::SendCompose, &ctx);
    assert_eq!(model.compose.as_ref().unwrap().stage, Stage::Sending);
    let requests = cmd.into_requests();
    assert_eq!(requests.len(), 1, "one command, not a chain");
    assert_eq!(requests[0].path, "/api/portal/rust-ui/signing/command");

    let sent = serde_json::json!({ "outcome": "success", "value": { "issued": {} } });
    LuxesignSigning::update(&mut model, Msg::ComposeStepped(Ok(sent)), &ctx);
    assert!(model.compose.is_none());
    assert!(model
        .notice
        .as_deref()
        .unwrap()
        .starts_with("Sent for signature"));
}

#[test]
fn a_refused_send_keeps_the_panel_and_says_why() {
    let ctx = ScreenCtx::default();
    let (mut model, _) = LuxesignSigning::init(&ctx);
    model.compose = Some(Compose {
        stage: Stage::Sending,
        ..Compose::default()
    });
    let refused = serde_json::json!({
        "outcome": "rejected",
        "error": { "message": "Page 5 does not exist: the document has 3 page(s)." }
    });
    LuxesignSigning::update(&mut model, Msg::ComposeStepped(Ok(refused)), &ctx);
    let compose = model.compose.as_ref().unwrap();
    assert_eq!(compose.stage, Stage::Idle);
    assert!(compose.error.as_deref().unwrap().contains("3 page"));
}

#[test]
fn status_buckets_match_the_metrics() {
    assert_eq!(EnvelopeStatus::of("completed"), EnvelopeStatus::Completed);
    assert_eq!(EnvelopeStatus::of("bogus"), EnvelopeStatus::Waiting);
}

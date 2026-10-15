// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

#[test]
fn session_loads_then_consent_unlocks_completion() {
    let ctx = ScreenCtx {
        id: Some("token-abc".into()),
        ..ScreenCtx::default()
    };
    let (mut model, cmd) = SignDocument::init(&ctx);
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/v1/signer/session");
    assert_eq!(model.token, "token-abc");

    let session = test_session(false);
    SignDocument::update(&mut model, Msg::SessionLoaded(Ok(session)), &ctx);
    // The open is recorded automatically on first sight.
    // Consent is a separate, deliberate act.
    assert!(!model.consent);
}

#[test]
fn working_guards_double_submit() {
    let ctx = ScreenCtx::default();
    let (mut model, _) = SignDocument::init(&ctx);
    model.token = "token-abc".into();
    model.session = Remote::Loaded(test_session(true));
    model.working = true;
    let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
    assert!(cmd.into_requests().is_empty());
}

fn test_session(consented: bool) -> SignerSession {
    SignerSession {
        signature_request_id: "req-1".into(),
        recipient: crate::model::SignerRecipient {
            id: "r-1".into(),
            signature_request_id: "req-1".into(),
            name: "Ada".into(),
            email: "ada@example.test".into(),
            role: "signer".into(),
            signer_order: 1,
            signing_step: 1,
        },
        state: "in_progress".into(),
        fields: vec![],
        consented,
        is_turn: true,
        expires_at: "2026-12-01T00:00:00+00:00".into(),
        ..SignerSession::default()
    }
}

fn field(id: &str, kind: &str, required: bool) -> crate::model::SignerField {
    crate::model::SignerField {
        id: id.into(),
        field_type: kind.into(),
        field_key: format!("{kind}-{id}"),
        page_number: 5,
        required,
        ..Default::default()
    }
}

fn loaded(session: SignerSession) -> (Model, ScreenCtx) {
    let ctx = ScreenCtx::default();
    let (mut model, _) = SignDocument::init(&ctx);
    model.token = "token-abc".into();
    model.session = Remote::Loaded(session);
    (model, ctx)
}

fn art() -> SignatureArt {
    SignatureArt {
        signature: "data:image/png;base64,SIG".into(),
    }
}

#[test]
fn choosing_a_style_redraws_the_preview_and_only_the_latest_answer_counts() {
    let mut session = test_session(true);
    session.fields = vec![field("s", "signature", true)];
    let (mut model, ctx) = loaded(session);

    // Choosing a style asks the browser to draw it, tagged with that style.
    let cmd = SignDocument::update(&mut model, Msg::SignatureStyle(2), &ctx);
    assert!(model.previewing);
    assert!(
        format!("{cmd:?}").contains("RenderSignature(Ada, style 2)"),
        "{cmd:?}"
    );

    // The drawing for the CURRENT style is kept and sent to the preview endpoint, pictures in the body.
    let cmd = SignDocument::update(&mut model, Msg::PreviewArt(2, Ok(art())), &ctx);
    assert_eq!(model.art_style, Some(2));
    assert_eq!(format!("{cmd:?}"), "PreviewPdf(/v1/signer/preview)");

    // A slow answer for a style the signer has since left must not replace the page they are looking at.
    SignDocument::update(&mut model, Msg::SignatureStyle(1), &ctx);
    SignDocument::update(
        &mut model,
        Msg::PreviewReady(2, Ok("blob:stale".into())),
        &ctx,
    );
    assert_eq!(model.preview_url, None);
    SignDocument::update(
        &mut model,
        Msg::PreviewReady(1, Ok("blob:fresh".into())),
        &ctx,
    );
    assert_eq!(model.preview_url.as_deref(), Some("blob:fresh"));
    assert!(!model.previewing && !model.preview_failed);
}

#[test]
fn a_preview_that_cannot_be_drawn_falls_back_to_the_full_document() {
    let (mut model, ctx) = loaded(test_session(true));
    SignDocument::update(&mut model, Msg::SignatureStyle(0), &ctx);
    let error = ApiError::network("no canvas");
    SignDocument::update(&mut model, Msg::PreviewArt(0, Err(error)), &ctx);
    assert!(model.preview_failed && !model.previewing);
}

#[test]
fn signing_uses_the_picture_the_preview_already_drew() {
    let mut session = test_session(true);
    session.fields = vec![field("s", "signature", true)];
    let (mut model, ctx) = loaded(session);
    model.art = art();
    model.art_style = Some(model.signature_style);
    // No redraw: straight into the chain (consented already, so the first request is the signature itself).
    let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
    let requests = cmd.into_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/signer/field");
    assert!(requests[0]
        .body
        .as_ref()
        .unwrap()
        .to_string()
        .contains("data:image/png;base64,SIG"));
}

#[test]
fn one_button_signs_everything_and_asks_for_the_drawing_first() {
    let mut session = test_session(false);
    session.fields = vec![
        field("s", "signature", true),
        field("i", "initials", true),
        field("d", "date", true),
    ];
    let (mut model, ctx) = loaded(session);

    // Not agreed yet: the button says why instead of sending anything.
    let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
    assert!(cmd.into_requests().is_empty());
    assert!(model.notice.as_deref().unwrap().contains("agree"));
    assert!(!model.working);

    // Agreed: the first thing that happens is the picture is drawn (a browser command, not a request).
    model.consent = true;
    let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
    assert!(model.working);
    assert!(
        format!("{cmd:?}").contains("RenderSignature(Ada, style 0)"),
        "{cmd:?}"
    );

    // The picture arrives: agree, then every block in order, then finish — queued as ONE chain.
    let cmd = SignDocument::update(&mut model, Msg::SignatureReady(Ok(art())), &ctx);
    assert_eq!(
        model.queue,
        vec![
            Step::Field("s".into()),
            Step::Field("i".into()),
            Step::Field("d".into()),
            Step::Complete
        ],
        "consent is the first request, already in flight"
    );
    let first = cmd.into_requests().remove(0);
    assert_eq!(first.path, "/v1/signer/consent");

    // Each success sends the next one, until the chain ends with complete and a re-read.
    let ok = serde_json::json!({ "outcome": "success" });
    let mut paths = Vec::new();
    for _ in 0..4 {
        let cmd = SignDocument::update(&mut model, Msg::Acted(Ok(ok.clone())), &ctx);
        paths.push(cmd.into_requests().remove(0).path);
    }
    assert_eq!(
        paths,
        vec![
            "/v1/signer/field",
            "/v1/signer/field",
            "/v1/signer/field",
            "/v1/signer/complete"
        ]
    );
    let cmd = SignDocument::update(&mut model, Msg::Acted(Ok(ok)), &ctx);
    assert_eq!(
        cmd.into_requests().remove(0).path,
        "/v1/signer/session",
        "then the page re-reads the server's truth"
    );
}

#[test]
fn a_refusal_stops_the_chain_and_says_why() {
    let mut session = test_session(true);
    session.fields = vec![field("s", "signature", true)];
    let (mut model, ctx) = loaded(session);
    model.queue = vec![Step::Complete];
    model.working = true;
    let refused =
        serde_json::json!({ "outcome": "rejected", "error": { "message": "Not your turn." } });
    let cmd = SignDocument::update(&mut model, Msg::Acted(Ok(refused)), &ctx);
    assert!(cmd.into_requests().is_empty());
    assert!(model.queue.is_empty() && !model.working);
    assert_eq!(model.notice.as_deref(), Some("Not your turn."));
}

#[test]
fn the_signature_travels_as_the_picture_drawn_and_initials_and_date_are_left_to_the_document() {
    let mut session = test_session(true);
    session.fields = vec![
        field("s", "signature", true),
        field("i", "initials", true),
        field("d", "date", true),
    ];
    let (mut model, _) = loaded(session.clone());
    model.art = art();
    model.signature_style = 2;
    let value = |id: &str| field_value(session.fields.iter().find(|f| f.id == id).unwrap(), &model);
    assert_eq!(value("s")["image"], "data:image/png;base64,SIG");
    assert_eq!(value("s")["style"], 2);
    assert!(
        value("i").get("initialsImage").is_none(),
        "initials are typeset, not pictures"
    );
    assert_eq!(value("i")["name"], "Ada");
    assert_eq!(value("d"), serde_json::json!({ "auto": true }));
}

#[test]
fn a_required_box_the_signer_must_fill_blocks_the_button_until_it_is() {
    let mut session = test_session(true);
    session.fields = vec![field("t", "text", true), field("s", "signature", true)];
    let (mut model, _) = loaded(session.clone());
    assert!(sign_blocker(&model, &session).unwrap().contains("complete"));
    model.values.insert("t".into(), "Casa del Mar".into());
    assert_eq!(sign_blocker(&model, &session), None);
    let mut waiting = session;
    waiting.is_turn = false;
    assert!(sign_blocker(&model, &waiting)
        .unwrap()
        .contains("earlier signer"));
}

#[test]
fn declining_is_two_steps_and_sends_the_reason() {
    let ctx = ScreenCtx::default();
    let (mut model, _) = SignDocument::init(&ctx);
    model.token = "token-abc".into();
    model.session = Remote::Loaded(test_session(true));

    // Opening the step sends nothing: it only asks.
    let cmd = SignDocument::update(&mut model, Msg::DeclineOpened, &ctx);
    assert!(model.declining);
    assert!(cmd.into_requests().is_empty());

    SignDocument::update(
        &mut model,
        Msg::DeclineReasonChanged("  Price is wrong  ".into()),
        &ctx,
    );
    let cmd = SignDocument::update(&mut model, Msg::DeclineSubmitted, &ctx);
    let requests = cmd.into_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/signer/decline");
    assert!(requests[0]
        .body
        .as_ref()
        .unwrap()
        .to_string()
        .contains("Price is wrong"));
}

#[test]
fn the_page_names_the_document_by_the_senders_subject_first_and_joins_the_line_with_spaces() {
    let mut session = test_session(true);
    session.document_title = Some("luxesign proof".into());
    assert_eq!(session.document_name(), Some("luxesign proof"));
    session.subject = Some("  Listing Agreement  ".into());
    assert_eq!(session.document_name(), Some("Listing Agreement"));
    assert_eq!(
        prepared_for(&session),
        "Prepared for Ada \u{b7} Open until 2026-12-01"
    );
    session.expires_at.clear();
    assert_eq!(prepared_for(&session), "Prepared for Ada");
}

#[test]
fn cancelling_a_decline_forgets_the_reason() {
    let ctx = ScreenCtx::default();
    let (mut model, _) = SignDocument::init(&ctx);
    model.declining = true;
    model.decline_reason = "x".into();
    SignDocument::update(&mut model, Msg::DeclineCancelled, &ctx);
    assert!(!model.declining && model.decline_reason.is_empty());
}

#[test]
fn complete_unlocks_only_when_every_required_field_has_an_answer() {
    let mut session = test_session(true);
    session.fields = vec![crate::model::SignerField {
        id: "f1".into(),
        required: true,
        ..Default::default()
    }];
    assert!(!session.fields_answered());
    session.answered_field_ids = vec!["f1".into()];
    assert!(session.fields_answered());
}

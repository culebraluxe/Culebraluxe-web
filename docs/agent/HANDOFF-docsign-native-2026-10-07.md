# Handoff — native document signing (the BoldSign replacement)

Written 2026-10-07 by the `claude` lane. Status: **built, tested end to end on DEV, one real production send done**.
The Forms "Send for signature" button has not yet been pressed by a person in production.

## The flow (what a person sees)

1. **Forms** (Portal → Core): the listing agreement is filled in; **Send for signature** issues a new version of the PDF
   and sends it. Issuance draws **Lisa's pre-signature, initials and date** into the PDF
   (`db/src/broker_signature.rs::resolve_for_issuance`, wired in `db/src/vault/bind_form_to_contract.rs`). Her block is
   recorded in `source_snapshot.render.appliedSignatures`, so the signing step leaves it alone.
2. **`documentSign.send`** (one atomic command, `web/src/document_sign/mod.rs::send_transactional`): prepare → place each
   signer's blocks → issue. Placement is `template` (the form's own anchors, `source_snapshot.render.signatureAnchors`),
   `lastPage`, or `page`. Issue refuses a field on a page the document does not have.
3. **The signer's email** (navy card on tan, logo from `public/images/culebraluxe-email-logo.png`) links to
   `<PUBLIC_SITE_URL>/sign/<token>`.
4. **The signing page** (`web/ui/src/app/screens/sign_document.rs`): read the document, choose one of four cursive faces
   (`public/fonts`, declared in `web/ui/styles/app.css`), tick the consent box, press **Sign & complete**. One press runs the
   whole chain (consent → each block → complete). The browser draws the signature and initials on a canvas
   (`Cmd::RenderSignature`, `app/exec/browser.rs`) and the *same pixels* are sealed into the PDF.
5. **Sealing is automatic**: the last signer's completion emits `DOCUMENT_SIGN_READY_TO_FINALIZE`;
   `web/src/document_sign/worker.rs::DocumentSignFinalizeSubscriber` runs `documentSign.finalize`: the sealed PDF (pictures,
   initials, the date in the brokerage's own format), a **certificate of completion**, then completion emails to every
   signer, anyone in `copyTo`, and the sender, with both files attached.
6. A **sweeper** (every 15 min, same file) expires overdue signers and sends reminders (every `reminderEveryDays`, default 3,
   at most 3).

## Production settings (Vercel project `culebraluxe-web-fp`, NOT `culebraluxe-rust-api`)

`ICLOUD_MAIL_ADDRESS`, `ICLOUD_MAIL_USERNAME`, `ICLOUD_SMTP_APP_PASSWORD` (mail); `PUBLIC_SITE_URL` (link + logo origin);
`DOCSIGN_ACCESS_SECRET` (signs links — links minted with any other secret are refused). Variables are snapshotted when a deploy
is created: add them *before* `pnpm deploy:prod`. The `BROKER_SIGNATURE_*` variables are optional overrides; by default the
signer is found by name and the image by `alt_text = broker_signature:lisa_penfield` (both exist in PROD).

## Traps found the hard way

- Minting a link from a laptop uses the laptop's secret. To test production mail, mint with the production secret.
- `documentSign.resend` once queued mail but never emitted the delivery event — every queued email needs its
  `email.delivery.requested` event (see the Resend/Finalize/Decline arms in `command_runtime.rs`).
- The seal must MERGE into the page's `/Font` and `/XObject` dictionaries (`signing_overlay.rs::merge_resource_entries`);
  replacing them removes the agreement's own text.
- PDF text is WinAnsi bytes, not UTF-8: keep `Vec<u8>`, never `String::from_utf8_lossy` (`pdf.rs`, `signing_overlay.rs`).
- The shared `CARGO_TARGET_DIR` is overwritten by concurrent lanes; use a lane-private one when results look impossible.

## Not built

Click-to-place fields on the PDF for ad-hoc documents (needs a PDF renderer in the browser); a "draw your signature" tab
(the canvas pipeline already accepts any PNG); automatic keep-warm if the container sleeps between requests.

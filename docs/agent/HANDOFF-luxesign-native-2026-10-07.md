# Handoff — native document signing (the BoldSign replacement)

Written 2026-10-07 by the `claude` lane. Status: **built, tested end to end on DEV, one real production send done**.
The Forms "Send for signature" button has not yet been pressed by a person in production.

## The flow (what a person sees)

1. **Forms** (Portal → Core): the listing agreement is filled in; **Send for signature** issues a new version of the PDF
   and sends it. Issuance draws **Lisa's pre-signature, initials and date** into the PDF
   (`db/src/broker_signature.rs::resolve_for_issuance`, wired in `db/src/vault/bind_form_to_contract.rs`). Her block is
   recorded in `source_snapshot.render.appliedSignatures`, so the signing step leaves it alone.
2. **`luxesign.send`** (one atomic command, `web/src/luxesign/mod.rs::send_transactional`): prepare → place each
   signer's blocks → issue. Placement is `template` (the form's own anchors, `source_snapshot.render.signatureAnchors`),
   `lastPage`, or `page`. Issue refuses a field on a page the document does not have.
3. **The signer's email** (navy card on tan, logo from `public/images/culebraluxe-email-logo.png`) links to
   `<PUBLIC_SITE_URL>/sign/<token>`.
4. **The signing page** (`web/ui/src/app/screens/sign_document.rs`): read the document, choose one of four cursive faces
   (`public/fonts`, declared in `web/ui/styles/app.css`), tick the consent box, press **Sign & complete**. One press runs the
   whole chain (consent → each block → complete). The browser draws the signature and initials on a canvas
   (`Cmd::RenderSignature`, `app/exec/browser.rs`) and the *same pixels* are sealed into the PDF.
5. **Sealing is automatic**: the last signer's completion emits `LUXESIGN_READY_TO_FINALIZE`;
   `web/src/luxesign/worker.rs::LuxesignFinalizeSubscriber` runs `luxesign.finalize`: the sealed PDF (pictures,
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
- `luxesign.resend` once queued mail but never emitted the delivery event — every queued email needs its
  `email.delivery.requested` event (see the Resend/Finalize/Decline arms in `command_runtime.rs`).
- The seal must MERGE into the page's `/Font` and `/XObject` dictionaries (`signing_overlay.rs::merge_resource_entries`);
  replacing them removes the agreement's own text.
- PDF text is WinAnsi bytes, not UTF-8: keep `Vec<u8>`, never `String::from_utf8_lossy` (`pdf.rs`, `signing_overlay.rs`).
- The shared `CARGO_TARGET_DIR` is overwritten by concurrent lanes; use a lane-private one when results look impossible.

## Not built

Click-to-place fields on the PDF for ad-hoc documents (needs a PDF renderer in the browser); a "draw your signature" tab
(the canvas pipeline already accepts any PNG); automatic keep-warm if the container sleeps between requests.

## 2026-10-08 — the preview pane draws her block through that same resolver (lane `deep`, `3adb21fc4`)

Status: **landed on `main`**, `3adb21fc4`. The defect: the Forms preview pane showed an UNSIGNED broker line while the
issued PDF carried Lisa's signature, initials and date. The pane was building its render request by hand and never
resolved the pre-signature at all, so one feature had two answers and the one on screen was not the one in the document.

Now there is one. `VaultDao::resolve_applied_signatures_in` (`db/src/vault/bind_form_to_contract.rs`) is the ONLY
resolution: issuance calls it inside its transaction with `SignatureAuthority::ApplyingActor` — the actor must be her or
a ROOT delegate, and a refusal fails the issuance — and the preview calls the connection-taking
`resolve_applied_signatures` with `OwnerByConstruction`: nothing is being applied there, so no application authority is
asked, while the policy, the declared-signer check, the slot mapping and the protected asset stay exactly the rules
issuance obeys. `SignatureAuthority` (`middle/model/src/forms_broker_signature.rs`) is a named enum and not a bool, so an
issuance cannot quietly ask for the weaker answer; `slots_from_signers` (`middle/model/src/forms_execution.rs`) is the
one slot construction both paths share, moved out of the issuance body.

Two things surfaced besides the pane:

1. **`issued_at` was `None` at both issuance sites** (`web/src/api/portal_bridge/forms_write_actions.rs`), and the
   resolver REFUSES an empty issuance instant — so a listing that names her was refused, not issued. Both sites now pass
   `issued_at_now()` (`web/src/api/portal_bridge/clients.rs`): the command instant, taken once at the command boundary.
   The pane resolves the same instant, so the date on screen and the date in the record are the same date. On a DRAFT
   that date is today and moves if the document is issued later — the only date an unissued document has.
2. **The preview's entitlement is `vault.read`, not `vault.issue`** — a person who may look at a draft is not
   necessarily a person who may sign it, which is exactly why the weaker authority is the preview's, and why it is
   named rather than implied.

Receipts (lane `deep`, base `8f64efab4`):

| command | result |
| --- | --- |
| `pnpm slice:check --since 8f64efab4` | T0 `cargo check --workspace --all-targets` PASS (35s) · FMT PASS (4s) · T1 section `app-core` PASS (110s, 17+50+121+18+155 tests) · T2 deliberately not run; `RESULT the slice may be handed over`, `EXIT=0` |
| `cargo test -p db` | 50 passed, 0 failed, exit 0 |
| `cargo test -p web` | 155 passed, 0 failed, exit 0 |
| `cargo test -p test-harness --test docs_forms_execution__001__fact_mapping` | 1 passed |
| `... --test docs_forms_execution__002__signer_selection` | 1 passed |
| `cargo test -p test-harness --test docs_forms_execution__003__broker_signature` | 1 passed |
| `... --test docs_forms_execution__004__applied_signature` | 1 passed |
| `... --test docs_forms_execution__005__issue_document` | 1 passed |
| `... --test docs_forms_execution__006__replay` | 1 passed |
| `... --test docs_forms_execution__007__immutable_issued_participant_slots` | 1 passed |
| `cargo fmt -p model -p db -p web -- --check` and `git diff --check` | clean |
| `git push origin HEAD:main` | `8f64efab4..3adb21fc4`, exit 0 |

Live rows, read-only against PROD (`$DATABASE_URL_PROD`, host `ep-flat-art-ax92tn7a-pooler.c-4.us-east-2.aws.neon.tech`):
exactly ONE active `app_user` named Lisa Penfield, exactly ONE `media` row with
`alt_text = broker_signature:lisa_penfield` and an image MIME type, and the four most recent `LISTING-01` instances
(v5, draft) carry `field_values->>'brokerName' = "Lisa Penfield"`, with 0 participant rows and her `app_user.person_id`
null. So the resolver's declared-signer check passes, its person-equality check is skipped and the slot is `None` — the
exact shape the already-issued PDFs are drawn from. That is the preview's inputs verified on the real rows; nobody has
yet pressed **Send for signature** in production, so the pane itself still awaits one human look.

Deliberately NOT done:

- **The V4 `LISTING-01` form instances were not deleted.** That is a destructive business-data change and nobody
  directed it in this session. For the record, `document_form_participant.form_instance_id` is
  `references document_form_instance(id) on delete cascade`
  (`db/migrations/066_forms_engine_context.sql:35-43`), so participants follow the instance and need no separate
  clearing, and an issued `transaction_document` is not removed with the draft it came from.
- **`web/ui/src/app/screens/forms/editor.rs:34`** (`template.version == template.active_version`) was left alone: it
  labels a chip ("Active" on v5, "History" on v4) and gates the "Fill Client" affordance. It never gates the preview
  request, so a v4 pane still renders her block.

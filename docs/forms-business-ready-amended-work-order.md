# Forms Presentation Engine — Amended Work Order

Status: implementation foundation exists on `codex/forms-business-ready-foundation`; Lisa-signature authorization is approved and an original transparent candidate asset has been generated. Lisa’s visual acceptance, protected-media configuration, human/provider/device proof, and legal-content acceptance remain open.

## Outcome

Make the four Forms portal products produce restrained, business-usable CulebraLuxe PDFs while preserving one canonical artifact path:

1. the editor previews the canonical renderer;
2. issuance stores immutable PDF bytes, SHA-256, source values, participant slots, and resolved signature geometry;
3. download, share, and signature workflows consume those stored bytes;
4. no downstream path silently regenerates a merely similar document.

The four primary portal products are:

| Product | Template ID | Presentation profile |
|---|---|---|
| Showing Report | `SHOW-RPT` | report |
| Offer Letter | `OFFER-01` | letter |
| Purchase & Sale | `PR-PNS` | agreement |
| Listing Contract | `LISTING-01` | agreement |

`SHOW-INFO` and `PR-PNS-AMD` are production templates too. They are secondary regression scope unless the Product Owner explicitly promotes them into the primary visual-acceptance set.

## Decisions and definitions

### Canonical artifact invariant

- A mutable draft preview uses the same renderer, values, sections, participant set, and next lineage version as issuance.
- A mutable preview is not promised to be byte-identical to a future issue if data or lineage changes before the issue command commits.
- Once issued, the stored Vault media bytes are canonical. Download, Web Share, and BoldSign must use those exact bytes.
- A source-content fingerprint may decide whether the current immutable version can be reused. It is not a security checksum; issued bytes retain SHA-256 evidence.

### Signature geometry

- The composition engine owns semantic signature layout and resolves geometry after pagination.
- Issuance snapshots rectangles in PDF points with a bottom-left origin, zero-based page index, page dimensions, role, slot, and field kind.
- Provider adapters perform provider-specific page/origin conversion exactly once.
- Missing or ambiguous geometry fails closed. Do not invent a “last page, lower-left” fallback.

### Legal and identity boundary

- “Business-ready presentation” does not mean attorney-approved legal content.
- Do not materially rewrite contractual meaning under a visual-design ticket.
- Product Owner authorization recorded 2026-08-26: Lisa Penfield is the sole application owner; her authenticated Google session plus the explicit human Issue action is the consent boundary for composing her brokerage signature into client-facing forms.
- Local issuance composition is the selected architecture because the same already-signed broker artifact must work for both BoldSign and direct Mail/Messages/Web Share delivery.
- Restrict automatic composition to the configured active owner, Lisa’s own protected signature asset, the template/role allowlist, and a form field that identifies Lisa as the broker occupying that role.
- Use the command `requestedAt` in `America/Puerto_Rico` as the issuance-date semantic. Re-rendering a draft may not alter an already-issued version.
- Every locally composed Lisa signature must display `Real Estate Broker License #: C-9931` immediately below Lisa's printed signer name. Treat this as signature-bound legal identity, not optional body copy.
- Disabling the feature or changing the protected media-id binding revokes future composition; prior issued versions remain immutable evidence.
- Do not create, trace, infer, or substitute a signature asset without explicit Product Owner/Lisa authorization. The 2026-08-26 original candidate was created under that explicit authorization; production activation still requires Lisa’s visual acceptance of the result.

## Work items

### FORMS-BR-00 — Resolve product decisions

Owner: Product Owner / legal / brand. Status: open.

Decide and record:

- whether the primary set is the four portal entries above or all six production templates;
- whether “Showing” means `SHOW-RPT`, `SHOW-INFO`, or both;
- whether Offer Letter is intentionally a concise term sheet or needs approved narrative copy;
- confirm the initial automatic-signature allowlist (`LISTING-01/SELLER_BROKER`, `PR-PNS/SELLER_BROKER`, `SHOW-INFO/BUYER_BROKER`, `SHOW-RPT/BUYER_BROKER`);
- approve the displayed English date format (`Month D, YYYY`) using the Puerto Rico calendar date at issuance;
- the attorney-approved content baseline for P&S and Listing.

Acceptance: decisions are written; unresolved choices are not silently implemented.

### FORMS-BR-01 — Canonical composition artifact

Status on foundation branch: implemented; formal review open.

- Keep one `TemplateDefinition -> RenderedFormPdfArtifact` composition boundary.
- Return bytes, page count/size, and provider-neutral signature anchors together.
- Use bounded presentation profiles (`agreement`, `letter`, `information`, `report`) rather than one renderer per form.
- Preserve deterministic metadata and byte output for identical inputs.
- Preserve Spanish/Puerto Rico Latin characters and smart-punctuation fallbacks.
- Keep logo loading compatible with the deployed server runtime.

Acceptance:

- identical inputs produce byte-identical PDFs;
- long unbroken values wrap without escaping bounds;
- all six templates parse and render;
- no plain database-style field dump is the document’s main hierarchy.

### FORMS-BR-02 — Composition and pagination quality

Status on foundation branch: implemented foundation; human approval open.

- A clearly legible CulebraLuxe logo on the right side of the header, restrained navy/gold system, readable serif body, sans labels, compact metadata footer, and Page X of Y.
- Lock the approved logo at its current right-side dimensions and coordinates. Do not resize or reposition it under this work order.
- Use exactly one document title: fixed left coordinate, fixed 12.5-point bold sans font, one line, and one baseline optically centered against the locked logo on every page of all six production templates. If a future title does not fit the fixed width, revise/approve the title copy; do not silently shrink, wrap, or move it.
- Keep the existing gold rule below the header. Do not repeat the form name below it. The small issuer and document-version line below the rule is audit metadata, not a second title, and must appear consistently on every form.
- Measured wrapping and semantic paragraph blocks.
- Agreement overview is bounded to core transaction facts.
- Keep each heading with at least the first two body lines.
- Keep the entire signature section together on a fresh page when it fits; otherwise paginate blocks without clipping.
- No signature field may collide with content or footer.

Acceptance: review representative PDFs for all four primary products at 100% and on paper/print preview. Record pass/fail by page.

### FORMS-BR-03 — Exact PDF preview in the editor

Status on foundation branch: implemented foundation; browser QA open.

- Replace the hand-built HTML facsimile with the actual PDF renderer.
- Debounce live POST rendering, abort stale requests, and revoke object URLs.
- Keep the last successful preview visible if a later render fails.
- Preview derives the next version from the same deal/template lineage query as issuance.

Acceptance:

- changing a field updates the PDF preview;
- no stale response replaces a newer preview;
- the current preview and a subsequent issue match in visible content and layout when inputs and lineage do not change;
- Chrome, Safari, and the supported iPad/iPhone path show a usable preview or an explicit fallback.

### FORMS-BR-04 — Immutable Vault/share parity

Status on foundation branch: implemented foundation; device QA open.

- Sharing must fetch the current issued document’s authenticated Vault download.
- If current draft content differs from the issued snapshot, save and issue first.
- If current content matches, reuse the current immutable document and avoid an unnecessary version.
- Never share a newly regenerated draft preview as though it were the issued document.

Acceptance:

- the SHA-256 of downloaded/shared input bytes equals the issued document checksum before platform handoff;
- autosave after an issue cannot cause sharing of a stale version;
- physical Safari/iOS Web Share is tested because desktop browser automation does not prove Mail/Messages attachment behavior.

### FORMS-BR-05 — Signature-anchor contract and BoldSign mapping

Status on foundation branch: offline implementation/tests complete; connected DEV proof open.

- Snapshot `signatureAnchors` and `pdfLayout` beside immutable issued source evidence; no schema migration is expected.
- Resolve an execution slot before provider dispatch.
- Convert canonical PDF bottom-left Y to provider top-left Y using `pageHeight - y - height` at the BoldSign boundary.
- Map semantic kinds to `Signature`, `Initial`, and `DateSigned`; set explicit sizes and approved date format.
- Reject legacy issued PDFs with no anchor metadata and instruct the operator to reissue.

Acceptance:

- unit tests prove bounds, page indexing, role/slot resolution, date-field serialization, and missing/ambiguous failure;
- a connected BoldSign DEV send proves the provider’s actual coordinate origin and renders each field inside its printed line;
- signed artifact reconciliation still appends bytes and preserves provider isolation/idempotency.

Do not call this item complete from coordinate documentation or mocks alone.

### FORMS-BR-06 — Template-specific presentation/content refinement

Status: pending Product Owner/legal review.

- Offer Letter: decide whether the approved product is a one-page terms sheet or a letter with approved narrative.
- Showing Report: confirm operational fields and whether a client/visitor signature is required.
- P&S: verify core-fact selection, clause order, party cardinality, and initial requirements.
- Listing: verify price/commission formatting, broker authority language, and signature roles.
- `PR-PNS-AMD` and `SHOW-INFO`: regression review, then either formally accept their profile output or add them to primary scope.

Acceptance: content owner initials each representative artifact. Presentation work must not be used as legal approval.

### FORMS-BR-07 — Broker automatic signature

Status: authorization and architecture approved; protected composition foundation implemented; real asset/configuration and live acceptance remain open.

Approved product behavior:

- Lisa’s authenticated Google session plus the explicit Issue action authorizes application of her brokerage signature.
- Compose Lisa’s signature and issuance date into the canonical PDF before its immutable media row and SHA-256 are created. That exact artifact then goes to Vault, direct Share, or BoldSign.
- Never apply it to buyer, seller, attorney, notary, another broker, an unassigned broker line, or any role outside the fixed allowlist.
- Remove only the already-satisfied Lisa signature/date fields from downstream provider anchors. External party fields remain normal provider-managed fields.
- Count a strictly parsed, slot-bound locally composed signature as provider-neutral agreement-execution evidence; never treat malformed or role-only snapshot data as evidence.

Implemented controls:

- feature is disabled until `BROKER_SIGNATURE_ENABLED=true` and the owner app-user id, protected media id, signer name, and broker license number are all configured;
- issuance verifies the command actor equals the configured active `owner` app user and, where both exist, the broker participant person id matches that owner;
- template id, declared signature group, broker role, signer field, and Lisa’s normalized name must all match;
- `PR-PNS` fails closed unless Lisa resolves to exactly one immutable `SELLER_BROKER` execution slot;
- asset bytes are loaded from the existing protected `media` store, accept PNG/JPEG only, and never live under `public/`;
- source evidence records actor, signer, `Real Estate Broker License #: C-9931`, role/slot, protected media id, asset SHA-256, applied instant, date semantic, consent basis, page, and resolved signature/date rectangles; the document row retains the final PDF SHA-256 and version;
- revocation affects future issuance by disabling the feature or changing/removing the configured media binding; issued versions are never rewritten.

Remaining operational steps:

1. obtain Lisa’s visual approval of the generated original transparent PNG candidate (or replace it with a signature she supplies);
2. upload the accepted asset as a protected image in the existing `media` store and configure the five environment names without committing values, including `BROKER_SIGNATURE_LICENSE_NUMBER=C-9931`;
3. prove one DEV issuance for every allowlisted template/role actually occupied by Lisa, including the Puerto Rico date around the UTC day boundary;
4. verify direct Share and BoldSign both receive the same issued checksum and that BoldSign contains only the external participants’ remaining fields;
5. exercise revocation and replacement, confirming prior issued PDFs remain byte-identical.

### FORMS-BR-08 — Golden artifacts and browser regression

Status: partly implemented as a review generator; durable baseline workflow open.

- Keep a deterministic review-artifact generator with representative accented names, long property values, and multi-party signatures.
- Add structural tests for page count, bounded anchors, deterministic bytes, and extracted critical text.
- Decide whether to commit visual goldens or produce them as CI artifacts; avoid brittle raw-PDF snapshots.
- Browser-test edit -> preview -> save -> download and dirty/autosaved -> share version selection.

### FORMS-BR-09 — Release gate

Status: open.

Required evidence:

1. targeted renderer, XML, issuance, and BoldSign adapter suites pass;
2. `pnpm typecheck` passes;
3. `git diff --check` passes;
4. `pnpm exec next build --webpack` passes;
5. four primary PDFs receive visual/product acceptance;
6. connected BoldSign DEV placement proof passes;
7. physical supported-device share proof passes;
8. legal/brand acceptance is recorded separately;
9. broker-signature readiness is green and live asset/identity/revocation evidence is recorded;
10. no production migration, deployment, or `main` merge occurs without explicit approval.

## Targeted verification commands

```bash
pnpm exec tsx --test \
  workflow_app/tests/forms-01-engine.test.ts \
  workflow_app/tests/doc08-xml-template.test.ts \
  workflow_app/tests/doc08-pns-pressure.test.ts \
  workflow_app/tests/doc06-issued-document.test.ts \
  workflow_app/tests/boldsign-adapter.test.ts \
  workflow_app/tests/broker-signature.test.ts \
  workflow_app/tests/agreement-participants.test.ts \
  workflow_app/tests/environment-readiness.test.ts

pnpm typecheck
git diff --check
pnpm exec next build --webpack
```

Generate representative artifacts individually so a failure identifies its template:

```bash
pnpm exec tsx scripts/generate-form-review-pdf.ts PR-PNS output/pdf/pr-pns-review.pdf
pnpm exec tsx scripts/generate-form-review-pdf.ts LISTING-01 output/pdf/listing-01-review.pdf
pnpm exec tsx scripts/generate-form-review-pdf.ts OFFER-01 output/pdf/offer-01-review.pdf
pnpm exec tsx scripts/generate-form-review-pdf.ts SHOW-RPT output/pdf/show-rpt-review.pdf
```

For an authorized local signature-placement review, pass the protected/candidate
PNG as the optional third argument after the output path. The generator reads
it transiently and does not copy it into the repository:

```bash
pnpm exec tsx scripts/generate-form-review-pdf.ts \
  LISTING-01 output/pdf/listing-01-review.pdf /secure/path/lisa-signature.png
```

## Branch and safety rules

- Start from current `origin/main`; work on a dedicated branch.
- Preserve unrelated changes and repository invariants.
- DEV only for connected integration proof. No production data writes.
- No schema change is expected for this pass. Stop and propose a migration separately if a real need emerges.
- Never log PDF contents, personal data, API keys, signature assets, or provider secrets.
- Do not merge to `main`, deploy, or run a production migration without explicit Product Owner approval.

## Current implementation handoff

The foundation branch already contains:

- a professional tokenized PDF composer shared by preview and issuance;
- presentation metadata on XML templates;
- corrected whitespace around inline XML `<value/>` substitutions;
- deterministic output and bounded signature geometry tests;
- issuance snapshots for participant slots, anchors, and page layout;
- exact PDF preview in the editor;
- issued-content fingerprinting for Vault/share version selection;
- Vault-byte sharing instead of regenerated preview sharing;
- provider-neutral anchor resolution and BoldSign coordinate conversion;
- explicit `Signature`, `Initial`, and `DateSigned` fields;
- representative PDF generation and visual inspection support.
- readable right-aligned logo composition rather than the rejected small/left treatment;
- issuance-bound protected broker-signature composition, identity/role allowlisting, immutable provenance, and local execution-slot evidence;
- non-secret production-readiness reporting for the broker-signature feature.

Grok should begin with the remaining FORMS-BR-00 content decisions, then FORMS-BR-06 through FORMS-BR-09 validation/refinement. FORMS-BR-07 is no longer blocked on authorization; it is blocked only on the real approved asset/configuration and live acceptance. Do not replace the canonical renderer, publish the signature asset, or reintroduce hardcoded provider coordinates.

## Provider references

- [BoldSign API getting started](https://developers.boldsign.com/api-overview/getting-started/)
- [Place form fields in a document](https://developers.boldsign.com/how-to-guides/place-form-fields-to-the-document/)
- [Set date format on a form field](https://developers.boldsign.com/how-to-guides/how-to-set-date-format-on-a-form-field/)

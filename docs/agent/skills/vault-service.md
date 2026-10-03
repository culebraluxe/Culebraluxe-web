# Skill: vault-service

The vault holds **issued transaction documents**: agreements, addenda, disclosures, titles
(`TransactionDocumentType`), each with a state, a version lineage and rendered artifacts.

- Domain vocabulary: `middle/model/src/vault.rs` — document types and states, source, create/issue/transition
  requests, `VaultActorScope`, render requests and rendered artifacts, command results.
- Persistence and commands: `db/src/vault.rs` with its own seams already split out —
  `bind_form_to_contract`, `listing_template_id`, `database`.
- Signature lineage: `db/src/broker_signature.rs`.
- Authority is an entitlement, not a role: reading is `vault.read`, writing is `vault.write`, issuing is
  `vault.issue` (and `documentSign.issue` / `documentSign.void` for the signing surface). The UI surface is the
  Cabinet at `/portal/documents`.
- An issued artifact is evidence. Do not rewrite an issued version in place — a correction is a new version through
  the transition path, so the lineage still explains what was issued and when.

## Anchored to
- `middle/model/src/vault.rs` — the vault vocabulary.
- `db/src/vault.rs` — the binding, with `bind_form_to_contract` / `listing_template_id` / `database`.
- `web/src/security/entitlement_catalog.rs` — `vault.read`, `vault.write`, `vault.issue`.

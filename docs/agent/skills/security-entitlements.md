# Skill: security-entitlements

An entitlement is a named `(code, operation_kind)` pair. Nothing else grants authority.

- Catalog: `rust/server/src/security/entitlement_catalog.rs` — `ACTIONS: &[(&str, &str)]`, e.g.
  `("contract.execute", "command")`, `("accounting.read", "query")`, `("vault.read", "query")`. An action that is
  not named there cannot be **decided**: the authorize endpoint refuses anything it cannot name, which is a catalog
  gap rather than a policy decision.
- Seeds and grants: the `entitlement` and `role_entitlement` tables. The Rust catalog and the SQL seed must agree —
  change both in the same commit, or the endpoint starts refusing a real action.
- Enforcement: `rust/core/service/src/authorization.rs`. Vocabulary: `rust/core/domain/src/security.rs`
  (domain side) and `rust/core/db/src/security.rs` (database side).
- Some authority is deliberately **not** a role grant. System-only operations (signer-edge, email delivery) stay
  narrowed in Casbin and are never inserted into `role_entitlement`. Do not "fix" that by adding a grant.
- The UI asks for codes, not roles: a screen's registration in `rust/ui/src/app/registry.rs` names the entitlements
  it requires (`deal.read`, `vault.read`).
- Widening a grant to make a test pass is the one change that always needs the human's eyes. Say so instead.

## Anchored to
- `rust/server/src/security/entitlement_catalog.rs` — the catalog the authorize endpoint reads.
- `db/migrations/261_docsign_entitlements.sql` — the matching seed, and its Casbin note on system-only authority.

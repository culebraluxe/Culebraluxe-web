# SECURITY-CORE-01 — Core Security Service

## Goal

Make Security a first-class CulebraLuxe Core service while preserving the
current simple access model:

`Google/Auth.js -> auth_identity -> app_user -> security_role -> Portal`

Broad security has exactly four application levels:

1. `ROOT`
2. `BUSINESS_POWER_USER`
3. `USER`
4. `GUEST`

Fine-grained entitlements are explicitly deferred.

## Scope

- Add `SecurityService` using the same `BaseService` kernel as Person, Firm,
  Property, Contract and Showing.
- Reuse the existing AUTH-02 SQL projection; no new security tables.
- Normalize current and legacy security-role codes into the four broad levels.
- Route canonical Portal identity mapping through `SecurityService`.
- Add broad security-level hooks to Portal navigation.
- Preserve existing authority metadata/checks for compatibility.
- Add an explicit permissive `EntitlementService` implementing the existing
  `AuthorizationPort` and wire it into the Core/Form service runtime.
- Register Security in the Core registry when its repository is supplied.
- Add targeted tests for level normalization, hierarchy, identity resolution,
  and the open entitlement stub.

## Current behavior preserved

- Portal still requires authenticated/mapped access through the existing
  server-side `portal.read` guard.
- Navigation hiding is presentation, not a new hard route boundary.
- Existing authority checks remain in place.
- The entitlement stub always allows; this story must not introduce new
  business-operation denials.
- TECH remains hidden unless the actor is ROOT and retains its existing
  `tech.access` compatibility check.

## Role compatibility

Current and transitional role codes collapse as follows:

- ROOT: `root`, legacy `owner`
- BUSINESS_POWER_USER: `business_power`, `business_power_user`,
  `bus_power_user`, legacy `agent`
- USER: `user`, `ops`, legacy `viewer`
- GUEST: `guest`, legacy `client`, unknown/empty

If multiple roles exist, the highest level wins. Unknown roles never elevate.

## Guardrails

- No Neon/PROD mutation.
- No migration.
- No main merge/deploy.
- No Listing Agreement mutation.
- No full regression.
- No fine-grained RBAC/ABAC policy in this story.
- No service-level business blocking beyond the existing application guards.

## Targeted assay

`pnpm exec tsx --test workflow_app/tests/security-core-service.test.ts`

Acceptance:
- exact Google/provider mapping remains fail-closed;
- mapped actor resolves through `SecurityService`;
- four levels are deterministic and hierarchical;
- unknown role resolves to GUEST;
- Portal actor snapshot carries the broad level;
- TECH has a ROOT-level visibility hook;
- Core/Form services receive an explicit entitlement port;
- entitlement stub remains open until the later entitlement story.

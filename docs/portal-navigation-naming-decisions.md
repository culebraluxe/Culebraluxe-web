# Portal Navigation Naming Decisions

Date: 2026-08-24

Authority: CTO / Product Owner

Status: Approved naming direction; implementation deferred until the remaining operating surfaces are reviewed.

## CORE

The user-facing display name for the current NEXUS operating surface will become **CORE**.

The intended Tier-2 navigation order is:

1. **Cockpit** — current Dashboard orientation surface (`/portal/dashboard`).
2. **Clients** — relationship context (`/portal/clients`).
3. **Catch-Up** — current Attention/follow-up surface (`/portal/attention`).
4. **Contracts** — current Deal/transaction surface (`/portal/deals`).
5. **Cabinet** — the user-facing home for Forms and Documents; the implementation seam and route remain to be decided.

### Product intent

- Clients precedes Catch-Up because relationships are more important than the system nagging Lisa.
- Contracts is the correct business term for the commitments represented by the current Deal surface.
- Cabinet means organized, usable paperwork rather than locked archival storage; it should encompass Forms and Documents.
- Workflows should operate beneath the visible product and surface only relevant steps or problems in context.
- The one-word, all-C naming system is intentional: memorable, plain, and bespoke to CulebraLuxe.

### Boundary

This records the approved user-facing vocabulary only. It does not authorize route, database, internal enum, authority, navigation-registry, or application-code changes by itself. Internal tokens such as `NEXUS` may remain stable even when the displayed label changes.

## OPPS

Naming review pending.

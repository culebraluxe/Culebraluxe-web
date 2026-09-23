# OPS-RECORDS-MEDIA-01 — OPPS becomes two tools

## Goal

Stop porting the eight-item OPPS menu. Rebuild the two jobs the captain actually runs, on the same MVI shape as CORE Clients:

1. **Listing Media** — pick a property, attach photos (`hero` | `gallery`), alt text.
2. **Records** — pick a property, archive / restore. Bounded ledger, not a generic table editor.

## Authority / scope

Captain call 2026-09-22. Surface stays `OPS` / label `OPPS` for this cut (rename later). Home becomes Records.

### In scope

- Nav: two listed items. Everything else on the OPS rail is `Retired` (routes stay, links go) — same move as TECH 2026-09-13.
- Yew components copied from `portal_clients.rs` (list rail + selected workspace + `PortalShell` + `on_msg`).
- Read model: one portal page DTO per screen, not `factRowsFrom`.
- Writes: archive/restore on Records; multipart upload on Listing Media via existing `POST /api/property-media/upload`.
- Screen keys stay `property-admin` and `property-media` so existing routes do not move.

### Out of scope

- Issue Queue, Needs Review, Media Audit, Identity Quality, Client Admin, Reporting, Decision Analysis — do not port bodies.
- People / deals ledgers on Records.
- Reorder / hero-from-gallery / unlink.
- Raw column editing of every property field.
- WASM artifact commit in this packet — local `pnpm ui:build:release` after the crate is green.

## The Clients pattern (copy this, do not invent a third)

See packet table in repo. Reducer stays pure. File bytes do not enter `Model`.

## Guardrails

- Do not add `_ => Vec::new()` on `Msg` to silence E0004.
- Do not serve `factRowsFrom(getPropertyAdmin())` to a ported Yew body.
- Allowed Records mutations this cut: `archive`, `restore`.
- Upload fields: `propertyId`, `role` (`hero`|`gallery`), `altText`, `file`.

## Wiring (smith order)

1. Types + Msg + Effect in model.rs
2. open() + match arms in update.rs; add keys to is_ported_portal_screen
3. yew_effects.rs read + command + multipart
4. Portal JSON loaders
5. Register views in mod.rs and yew_portal.rs
6. Registry: two OPS items, home /portal/property-admin
7. cargo check -p ui && pnpm ui:build:release

## Acceptance

Captain can archive one listing and attach one gallery photo without TypeScript workspaces. No E0004. No stale wasm.

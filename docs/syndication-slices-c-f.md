# Marketing slices C–F — DeepSeek work order

Repo: `culebraluxe/Culebraluxe-web`  
Branch: **extend** `feat/marketing-syndication` (do not invent a new nav world).  
Already on that branch: honesty pass, source_hash, sightings, expire, renew, off-market banner, needs-me filter, Launch checklist.  
Also read: `docs/syndication-v3-work-order.md`, `docs/syndication-adapters-requirements.md`

Finish leftover V2/V3 polish only if you trip over it. Then implement **C, D, E, F in that order**. Ship C+D even if E/F stall.

## Non-negotiable

- No Zillow / Realtor.com / Homes.com Prepare or POST.
- No HubSpot writes. No dual-write of contacts.
- No Clasificados or Matrix RPA.
- No new top-level OperatingShell world.
- Glass UI only.
- `SYNDICATION_LIVE` stays off unless a test sets it. Default dry-run.
- Public listing URL is `https://culebraluxe.com/listings/{slug}`.
- Office name is `CulebraLuxe`. Country PR. Default city Culebra.

## Slice C — Seller presence one-pager

Pure function `buildPresenceReport(source, placements, sightings)` in `lib/syndication/presence-report.ts`: name, price, city, facts, site URL, site published, Stellar status + MLS#/URL, Facebook URL, Clasificados URL, sightings by network, optional DOM, generated-at America/Puerto_Rico.

Disclaimer: portals update from Stellar; this lists confirmed or observed URLs; not an upload log. Never say “Published to Zillow.” Empty Zillow = “Not observed yet.” Stellar missing = “Not in Matrix yet.”

UI: **Seller report** on the selected listing. Printable glass panel + Copy + `window.print()`. PDF optional.

## Slice D — Share blurbs + QR

`lib/syndication/share.ts`: `whatsappBlurb(source, 'es'|'en')`, `smsBlurb(source, 'es'|'en')` with name, price, city, beds/baths, public URL, CulebraLuxe.

Workbench **Share** panel: copy WhatsApp ES/EN, SMS ES/EN, optional `wa.me/?text=`.

QR PNG/SVG of the **public** listing URL only. Reuse an in-repo QR helper if one exists; else `GET` portal route gated by `portal.read`. No portal-URL QR.

## Slice E — Public listing inquiry → CRM

Inspect first. Reuse existing person/inquiry/lead tables. If a public listing form exists, attach `property_id` and link the Client record from the Launch panel. If there is no public form, stop and write a short note in `docs/` — do not invent a marketing-only form.

No HubSpot. Public POST must not write syndication placements.

## Slice F — Meta live POST (guarded)

Audit existing Facebook adapter: dry-run without `SYNDICATION_LIVE`, zero fetch without photos, Page feed without catalog, Bearer header, persist external id/URL, `ok: false` on non-2xx. Add workbench readiness strip if missing. Warning when `readyToPost`: **Live Graph POST is armed.** No UI toggle that writes the env var. No items_batch. No ads.

Keep existing syndication tests green.

## Order

1. C  2. D  3. E (or blocked note)  4. F audit

## Lisa demo after C+D

Casa Luar → Launch checklist → Seller report print → Copy WhatsApp ES → Zillow is still a pasted sighting.

## Out of scope

IDX inbound, HubSpot, Advantage+, Socio, eXp, scraping Zillow, new OperatingShell surfaces, public-site redesign.

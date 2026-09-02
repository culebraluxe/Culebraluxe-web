# Syndication V3 work order — DeepSeek

Repo: `culebraluxe/Culebraluxe-web`  
Branch: `feat/marketing-syndication` (extend it; do not invent a third surface)  
Also read: `docs/syndication-adapters-requirements.md`, `docs/syndication-v2-remaining.md`  
Architect: Grok  
Product: Chris / CulebraLuxe (PR licensed brokerage, Lisa will use this screen)

Finish any leftover V2 items in `docs/syndication-v2-remaining.md` first, then do this document in order.

---

## 0. Product truth (do not violate)

CulebraLuxe cannot upload listings to Zillow or Realtor.com. Those sites ingest an MLS feed.

Flagship path:

1. `property` on culebraluxe.com is the source of truth (`is_published`).
2. PRAR + Stellar Matrix is the only legal entrance to Zillow / Realtor.com / Homes.com / co-broke.
3. Facebook Page / catalog is the only off-site path this app may POST later (tokens + `SYNDICATION_LIVE`).
4. Marketplace consumer cards and Clasificados stay paste + confirm.
5. HubSpot, Amplia, eXp, Island Homes, MyState, Clasificados Socio are out of scope.

If a control looks like “publish to Zillow,” Lisa will press it. That is a product bug.

---

## 1. UI honesty pass (do this first — Lisa-safe)

### Selectable channels (checkbox + Prepare)

- `culebraluxe` — info only; live state is `property.is_published`
- `stellar_mls` — pack / RESO checklist
- `facebook_marketplace` — pack + optional Graph Page/catalog
- `clasificados` — pack

Default selected: `stellar_mls` + `facebook_marketplace`.

### Not selectable — “Reaches via Stellar” strip

Move these out of the checkbox grid:

- Zillow
- Realtor.com
- Homes.com

Copy:

> Zillow, Realtor.com, and Homes.com are not upload targets. They update when this listing is live in Stellar and distribution is on. Paste a public URL later to pin it on the constellation.

`zillow_fsbo` and `realtor_com` stay in the channel enum / ledger so old rows validate. They must not be checkboxes. `pr_mls` stays blocked legacy. Hide Amplia and HubSpot behind “More (not this quarter),” default collapsed.

Button label: **Prepare selected**  
Helper line: **Does not upload to Zillow or Realtor.com.**

### Constellation language

- Site live → “On culebraluxe.com”
- Stellar pending → “Enter in Matrix, then paste MLS #”
- Stellar live → “In Stellar — portals follow the feed”
- Facebook pending → “Pack ready”
- Facebook live → “Page or Marketplace URL confirmed”
- Observed Zillow URL → “Seen on Zillow” (never “Published to Zillow”)

---

## 2. eXp-shaped features that are germane

eXp is not a Zillow API. They have one listing, many destinations, status, photos, office branding, sold cascade, and “where did this hit.” Steal only that.

### 2.1 Root change detection (stale pack)

When price, beds, baths, `is_published`, name, or hero photo changes, mark non-site placements as needing refresh. Add `source_hash` on `listing_syndication_placement`. Banner: **Price/facts changed — regenerate pack.** Keep `external_url` / `external_id`.

### 2.2 Off-market cascade

When the property is unpublished, sold, withdrawn, or archived: banner to take Facebook/Clasificados down; banner to update Matrix. Do not auto-delete third-party ads. Do not write Matrix.

### 2.3 Observed destinations (cheap Zillow tracker)

Table `listing_syndication_sighting` (property_id, network in zillow/realtor_com/homes_com/other, url, noted_at, notes). No Prepare button. Constellation chip: “Seen on Zillow.”

### 2.4 Photo packet

Copy-all photo URLs. Zip only if `media.file_data` makes it a half-day or less. Matrix: upload files, do not hotlink.

### 2.5 Expiry and renew

`expireStalePlacements` already exists. Add Renew on expired Clasificados/Facebook (`event_type` `renewed` already allowed).

### 2.6 Office lock

ListOfficeName = CulebraLuxe. Origin = https://culebraluxe.com. Country PR. Default city Culebra. Show office + agent on the Stellar pack.

### 2.7 Needs me filter

All / Needs me / Live / Expired. Needs me = pending_manual + stale hash + expired.

---

## 3. Will not build

Zillow/Realtor upload, IDX inbound, Clasificados/Matrix RPA, HubSpot dual-write, Socio/eXp/Island Homes, scraping to verify Zillow.

---

## 4. Order

1. V2 remaining if still open  
2. Honesty pass (§1) — Lisa demo gate  
3. Fingerprint (§2.1)  
4. Sightings (§2.3)  
5. Off-market banner (§2.2)  
6. Renew (§2.5)  
7. Photo zip only if cheap  

Migration `101_syndication_sightings_and_hash.sql`.

Tests: selectable list excludes zillow_fsbo and realtor_com; hash changes with price; sighting does not create a placement; renew event; Facebook dry-run zero fetch.

## 5. Lisa demo

Casa Luar → Prepare Stellar + Facebook → show office name and photos → say Zillow is not a button → confirm Facebook only with a real URL.

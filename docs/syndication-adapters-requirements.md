# CulebraLuxe syndication adapters — implementation spec (non-HubSpot)

Status: architect brief for DeepSeek / implementer  
Owner of this spec: Grok (architect lead)  
Product owner: Chris (CulebraLuxe broker + Tech Provider app owner)  
Repo: `culebraluxe/Culebraluxe-web`  
Branch to extend: `feat/marketing-syndication` (do not invent a parallel surface)  
Out of scope: HubSpot dual-write, Clasificados RPA, eXp, Amplia as flagship, Zillow FSBO as primary

---

## 0. Intent

Canonical listing lives in Neon `property` + `property_media` and is public on `https://culebraluxe.com/listings/{slug}` when `property.is_published = true`.

Syndication is **1 property → N placements**. Each placement is a row in `listing_syndication_placement` with an append-only `listing_syndication_event` ledger.

We do **not** scrape, do **not** drive third-party UIs, do **not** spam. Real brokerage accounts only.

Chris is a Meta Tech Provider and Business-verified, but is new to the Meta developer portal. **Put every durable artifact on our side of the house.** Chris only does one-time clicks in Meta that no API can replace (create catalog, grant Page token, paste three IDs into Vercel). DeepSeek must not stall waiting for those clicks: every adapter must dry-run with a complete payload when env is missing.

---

## 1. Current production state (2026-09-02)

Live in prod (first slice):

- Marketing world in `OperatingShell` (`/portal/marketing`, `/portal/marketing/syndication`)
- Channel catalog + pack generator + server actions
- Ledger tables exist in Neon after combined 098+099
- UI shows: site, Clasificados, Facebook Marketplace (pack), PR Realtors MLS (blocked), Amplia stub, Zillow FSBO pack, Realtor.com blocked, HubSpot stub
- Dashboard zeros until first pack is generated

On `feat/marketing-syndication` (not necessarily what prod screenshot shows):

- `lib/syndication/{channels,types,pack,adapters,env,facebook,stellar,hubspot}.ts`
- `runAdapter` is async
- Facebook builds Graph `home_listings` + Page `/feed` + partner `items_batch` payloads
- Stellar builds RESO Property + Matrix distribution checklist
- Live POST only if `SYNDICATION_LIVE=true` **and** tokens present
- Workbench selectable defaults: Facebook + Stellar (if that commit is deployed)

Implementer must `git pull` `feat/marketing-syndication`, diff against prod, and land **one** coherent branch. Do not fork a third catalog.

---

## 2. Target stack after this work (non-HubSpot)

| Channel id | Mode | After this spec | Human still required? |
|---|---|---|---|
| `culebraluxe` | api | already live | Property Admin publish toggle |
| `facebook_marketplace` | api + pack | Page feed + catalog home_listing POST when live; Marketplace consumer card remains pack+confirm | Create catalog + Page token once; confirm Marketplace URL if they also paste a consumer card |
| `stellar_mls` | mls / pack | RESO payload + Matrix checklist + confirm MLS id | Enter listing in Matrix once; tick distribution |
| `clasificados` | copy_pack | pack + photo zip hint + 40-day TTL + confirm URL | Paste ad |
| `zillow_fsbo` | copy_pack | fallback pack only | only if Stellar feed has not appeared |
| `realtor_com` | blocked | stays blocked; notes say "via Stellar distribution" | none |
| `pr_mls` | blocked | legacy id; do not delete (ledger check constraint) | none |
| `amplia_mls` | stub | leave stub | none |
| `hubspot` | out of scope | do not expand | none |

---

## 3. Architecture rules (non-negotiable)

1. **Source of truth is `property`.** Adapters read `ListingSource`. They never invent price/beds/city.
2. **Adapters are pure-ish functions + optional HTTP.**  
   `runAdapter(source, channel): Promise<AdapterResult>`  
   Always returns a pack. May attach `pack.transport`.
3. **Dry-run by default.**  
   `SYNDICATION_LIVE !== 'true'` ⇒ never call Meta. Still persist the exact JSON we would have POSTed.
4. **No secrets in `pack` jsonb.** Strip tokens. Store endpoint, method, payload, missingEnv, httpStatus, body (redact access_token query params).
5. **Idempotent ledger.** Unique `(property_id, channel)`. Re-prepare overwrites pack + last_attempt_at, does not duplicate rows.
6. **Photos are first-class.** Do not hotlink `culebraluxe.com` into Clasificados. Do not send the listing HTML URL as `image_url` to Meta. Resolve real image URLs from `property_media` ⋈ `media`.
7. **Glass UI only.** Reuse `PageHeader`, `Panel`, `CopyButton`, `STATUS_TONE`. No new design system.
8. **Server actions stay behind `portal.read`.** Same pattern as `app/portal/marketing/actions.ts`.
9. **English + Spanish packs stay.** Facebook/Page use EN. Clasificados uses ES. Stellar RESO fields are English dictionary names with Spanish public remarks allowed in `PublicRemarks`.

---

## 4. Data model

### 4.1 Already exists (do not recreate)

`listing_syndication_placement`  
`listing_syndication_event`  
Channel check must include `stellar_mls` (migration 099). If a fresh env only has 098, re-run the combined 098+099 script.

### 4.2 Required schema delta (new migration `100_syndication_media_and_external.sql`)

Add only if missing after inspect:

```sql
-- placement already has external_id, external_url, pack, expires_at

alter table listing_syndication_placement
  add column if not exists photo_manifest jsonb not null default '[]'::jsonb;

-- Prefer reusing pack_generated + detail.dryRun = true to avoid another constraint dance.
```

`photo_manifest` shape:

```ts
type PhotoManifestItem = {
  mediaId: string
  url: string          // absolute https URL Meta/Clasificados can fetch
  role: 'hero' | 'gallery'
  sortOrder: number
  width?: number
  height?: number
  contentType?: string
}
```

### 4.3 ListingSource expansion

Today `db/syndication.ts` `mapSource` / SQL does **not** select lat/lng, year_built, postal_code, street, or media URLs.

DeepSeek must extend `ListingSource` and the SQL in `listListingSources` / `getListingSource`.

Required new optional fields (null if column absent — inspect `property` and `media` first, do not assume names):

```ts
latitude?: number | null
longitude?: number | null
yearBuilt?: number | null
postalCode?: string | null
streetAddress?: string | null
photos: PhotoManifestItem[]
```

Photo resolution:

- Join `property_media` → `media` where `media_type = 'image'`
- Order hero first, then `sort_order`, `created_at`
- Cap at 25 images
- URL construction: use whatever the site already uses for public listing images. **Grep the codebase** for existing public media URL helpers. Do not invent a second CDN convention.
- If only private URLs exist, add a server-side signer or public path.

Fallback coordinates if lat/lng null: Culebra `18.303, -65.304`. Always send country `PR`, region `PR`, city default `Culebra`.

---

## 5. Env / config (our side)

All server-only. Never `NEXT_PUBLIC_`.

```
SYNDICATION_LIVE=false          # flip true only after one dry-run inspected
META_GRAPH_VERSION=v21.0
META_ACCESS_TOKEN=             # Page token from Tech Provider app, long-lived
META_PAGE_ID=
META_PRODUCT_CATALOG_ID=
META_AD_ACCOUNT_ID=            # optional, ads later
PUBLIC_MEDIA_ORIGIN=           # only if media URLs are relative
```

Add a tiny `lib/syndication/env.ts` reader (already exists on the feature branch).  
Add a **read-only readiness panel** on `/portal/marketing` or syndication workbench. Do not display token values. Only present / absent.

Chris TODO for Meta (the only portal work — isolate):

1. Meta Business Suite → Commerce Manager / Catalogs → create catalog type **Home listings** named `CulebraLuxe Listings`.
2. Copy catalog id → `META_PRODUCT_CATALOG_ID`.
3. Page that will publish → copy numeric `META_PAGE_ID`.
4. Tech Provider app → generate Page access token with `pages_manage_posts`, `pages_read_engagement`, `catalog_management` (or current 2026 equivalent). Store as `META_ACCESS_TOKEN` in Vercel Production + Preview.
5. Leave `SYNDICATION_LIVE=false` until first dry-run pack looks right.

DeepSeek implements everything else so that when those three strings land, POST works without another code change.

---

## 6. Adapter contracts

File map (keep this layout):

```
lib/syndication/channels.ts
lib/syndication/types.ts
lib/syndication/pack.ts
lib/syndication/env.ts
lib/syndication/photos.ts        # NEW
lib/syndication/facebook.ts
lib/syndication/stellar.ts
lib/syndication/clasificados.ts  # NEW
lib/syndication/adapters.ts
db/syndication.ts
app/portal/marketing/actions.ts
components/portal/marketing/*
workflow_app/tests/syndication-adapters.test.ts
```

### 6.1 TransportAttempt

```ts
type TransportAttempt = {
  kind: string
  dryRun: boolean
  liveEnabled: boolean
  method: 'POST' | 'GET' | 'MANUAL'
  endpoint: string
  payload: Record<string, unknown>
  missingEnv: string[]
  response?: Record<string, unknown>
}
```

Persist on `pack.transport`. Workbench must render it as a copyable `<pre>` labeled “API transport”.

### 6.2 Facebook — our house vs Meta house

**Our house (implement now):**

A. `buildFacebookHomeListingPayload(source)`  
Endpoint: `POST https://graph.facebook.com/{ver}/{META_PRODUCT_CATALOG_ID}/home_listings`

Minimum fields:

```
home_listing_id        = property.id
name
availability           = 'for_sale'
currency               = 'USD'
price                  = listPrice number
url                    = https://culebraluxe.com/listings/{slug}
description            = publicRemarks || shortDescription
num_beds, num_baths
property_type          map: villa/house→house, condo/apart→apartment,
                       land/lot/solar→land, town→townhouse
listing_type           = 'for_sale_by_agent'
year_built             if known, else omit (do not send 0)
address.street_address, city, region='PR', country='PR'
address.latitude, longitude
address.neighborhoods  = [neighborhood] or ['Culebra']
images[]               = [{ image_url }] from photo_manifest, hero first
```

B. `buildFacebookPageFeedPayload(source)`  
Endpoint: `POST https://graph.facebook.com/{ver}/{META_PAGE_ID}/feed`

```
message  = title EN + short remarks + beds/baths/city + "CulebraLuxe"
link     = public listing URL
published = true
```

C. `buildFacebookMarketplaceItemBatch(source)`  
Payload only. Do **not** POST unless `META_MARKETPLACE_PARTNER=true` is explicitly set. Default off. Consumer Marketplace create stays a paste target: `https://www.facebook.com/marketplace/create/item`.

D. `maybePostFacebook(transport)`

Order when live:

1. POST home_listings (catalog)
2. POST page feed
3. Never POST items_batch unless partner flag on

**Page feed 2xx ⇒ status `live`**. Catalog-only 2xx without feed ⇒ `pending_manual`. Dry-run ⇒ `pending_manual`. Both fail when live ⇒ `failed`.

Auth: `Authorization: Bearer ${token}` header. Strip tokens before persisting pack.

E. Image constraint  
If `photos.length === 0`, dry-run with a warning. When `SYNDICATION_LIVE=true` and no photos, **do not POST**.

### 6.3 Stellar / PRAR — pack only, never HTTP write

There is no broker write API. RESO Web API / Bridge / MLS Grid are pull/IDX.

Emit RESO Property + Matrix checklist + distribution flags (realtorCom, homesCom, homesnap, listHub true; zillowRentals false). OriginatingSystemName=`MFR`. Status after prepare: `pending_manual`. Never `live` until confirm.

### 6.4 Clasificados — pack only

Paste target: `https://www.clasificadosonline.com/Usuarios.asp`  
Title ES, body ES. TTL 40 days. Do not hotlink photos.

### 6.5 Zillow FSBO

Leave pack. Prefer Stellar. Do not build Zillow API.

### 6.6 Site adapter

Unchanged: `is_published` → live / draft.

---

## 7. Application layer

`requestPublish` must `await runAdapter`. Persist pack including transport. Sequential `requestPublishMany`.

Workbench SELECTABLE: `facebook_marketplace`, `stellar_mls`, `clasificados`, `zillow_fsbo`. Default selected: Facebook + Stellar. Show transport JSON. Meta env readiness present/absent only.

---

## 8. Jobs / hygiene

Expire `live` Clasificados/Facebook placements when `expires_at < now()`. If no cron, call `expireStalePlacements()` from the marketing dashboard GET.

---

## 9. Tests

`workflow_app/tests/syndication-adapters.test.ts`

- Facebook payload city Culebra, country PR, images from photos when present
- Dry-run: no fetch when env missing
- Live guard: SYNDICATION_LIVE=true but no photos → zero fetch calls
- Stellar liveEnabled false, realtorCom true, MFR
- Clasificados titleEs/bodyEs, ttlDays 40
- Channel catalog includes stellar_mls
- Do not regress navigation-registry MARKETING surface

Mock fetch. Never hit graph.facebook.com from CI.

---

## 10. Implementation order for DeepSeek

1. Inspect `property`, `media`, `property_media` and existing public image URL helper.
2. Extend `ListingSource` + SQL + `photos.ts`.
3. Align `channels.ts` with Stellar as first-class; keep `pr_mls` blocked legacy.
4. Harden `facebook.ts`: real photos, Bearer token, dual POST plan, no items_batch by default.
5. Harden `stellar.ts` + Clasificados instructions.
6. Ensure `requestPublish` awaits adapter and stores transport.
7. Workbench: selectable Stellar, transport pre, env readiness (no secrets).
8. Tests.
9. Migration 100 only if photo_manifest column wanted; otherwise keep photos inside `pack`.
10. Keep this file as the spec. Do not invent a second catalog.

Do **not** implement HubSpot writes. Do **not** add Playwright against Clasificados. Do **not** add Meta OAuth in v1.

---

## 11. Acceptance (demo-quality)

Given Casa Luar published, no Meta env: prepare Facebook + Stellar + Clasificados → three placement rows, dry-run transport, no Graph HTTP.

Given tokens set and `SYNDICATION_LIVE=false`: still no Graph HTTP, missingEnv empty.

Given `SYNDICATION_LIVE=true` + token + page + catalog + photos: POST attempted, httpStatus recorded, last_error on non-2xx.

Given Clasificados URL confirmed: status live, confirmed_at set.

---

## 12. Explicit non-goals

- HubSpot CRM object sync
- Meta OAuth UI in the portal
- Advantage+ ad campaign creation
- Marketplace partner paperwork
- Stellar RESO pull/IDX onto culebraluxe.com
- LoadRunner / Playwright against clasificadosonline.com
- Changing public site design

---

## 13. Chris-only checklist (off the engineering critical path)

- [ ] Home listings catalog created; id in Vercel `META_PRODUCT_CATALOG_ID`
- [ ] CulebraLuxe Page id in `META_PAGE_ID`
- [ ] Long-lived Page token in `META_ACCESS_TOKEN`
- [ ] `SYNDICATION_LIVE` left `false` until one dry-run pack is inspected
- [ ] PRAR + Stellar office membership
- [ ] First Matrix listing typed from the Stellar pack

Everything else is code in this repo.

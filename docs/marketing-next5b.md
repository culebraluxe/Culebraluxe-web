# Marketing next 5b — DeepSeek work order

`main` is live. Chris QAd the previous batch.

**Branch:** `feat/marketing-next5b` cut from current `origin/main`. PR into `main`. No force-push. No `SYNDICATION_LIVE` flip.

If `feat/marketing-syndication` still exists locally, merge `origin/main` into it or abandon it — `main` is source of truth.

No HubSpot. No Zillow/Realtor Prepare or POST. No new OperatingShell world.

## Task 1 — Placement activity timeline

`listing_syndication_event` is append-only and unused in the UI. On each workbench pack card, disclosure **Activity**: last ~10 events (type, America/Puerto_Rico time, short message). Read-only. No new table.

## Task 2 — Stale pack shows what changed

Migration `103_placement_source_snapshot.sql`: nullable jsonb `source_snapshot` on `listing_syndication_placement` (price, beds, baths, name, published) written at prepare. Stale banner lists diffs (`Price $2.5M → $2.35M`). Regenerate overwrites snapshot + hash, keeps external_url/id. If 103 missing, old banner, no 500.

## Task 3 — Seller report Spanish

`presenceReportText(report, 'es'|'en')`. EN|ES toggle on the panel. Same disclaimer meaning. Never “Published to Zillow”. Empty sightings ES: not observed.

## Task 4 — Launch completeness score

0–100 on the Launch panel. Weights: published 20, hero 15, ≥5 photos 15, price 15, city 10, Stellar confirmed 15, Facebook confirmed URL 10. Clasificados and sightings do not score. Hint lists what is missing.

## Task 5 — Just-sold / off-market share blurb

When off-market, extra EN/ES copy buttons. Unpublished → inventory root `https://culebraluxe.com`, not a dead listing URL. Do not auto-post Facebook.

## Order

1 (no migration) → 2 (103) → 3 → 4 → 5.

Existing syndication tests stay green. PR to `main`.

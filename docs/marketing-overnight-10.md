# Marketing overnight 10 — DeepSeek work order

`main` is live with migrations 098–103 on PROD.

**Branch:** `feat/marketing-overnight` from `origin/main`. PR into `main`. No force-push. No `SYNDICATION_LIVE`. No HubSpot. No Zillow/Realtor Prepare or POST. No new nav world.

Do not rebuild v1. Extend it. One migration max (`104_…sql`) if needed.

## 1. Stellar confirm is an MLS# field

Stellar card: MLS # (reject http/zillow/realtor URLs). Optional Matrix URL separately. `external_id` = MLS#, `external_url` = portal URL.

## 2. No Facebook/Clasificados Prepare when off-market

Reject those channels in the action if `isOffMarket`. Disable checkboxes. Stellar pack still allowed.

## 3. Facebook needs photos

Workbench banner if imageCount === 0. Live attempt stays false without photos.

## 4. Photo zip for Matrix

Gated zip of media bytes, cap 25. If no bytes in `media`, Copy photo URLs only + note in `docs/marketing-overnight-notes.md`. No CDN scrape.

## 5. Dashboard Needs-me is global

All listings with pending/expired/stale or score < 70. Jump to workbench with propertyId.

## 6. Sighting dedupe

Same property+network+url → no second row. Unique index only if you spend the one allowed migration on this.

## 7. One-page flyer (print)

Hero, facts, QR, CulebraLuxe, agent. EN/ES. Not the seller status report. No Zillow branding.

## 8. Price trail (derived)

Snapshot vs current price. No new history table unless one already exists.

## 9. ES chrome on Launch + Needs-me + off-market banner + Prepare helper

Negative Zillow sentence only. Do not i18n the whole portal.

## 10. Cron expire route

Reuse existing cron-secret pattern. Document if the repo has no vercel cron yet. Do not invent platform config.

## Order

1–3 guards → 6 → 5 → 7 → 8 → 9 → 4 → 10.

Existing tests stay green. PR to `main`.

# Marketing next 20 — DeepSeek work order

Finish PR #12 holes first, then 17 more stories.

**Branch:** `feat/marketing-next20` from `feat/marketing-overnight` if #12 is unmerged, else `origin/main`. PR to `main`. No force-push, no `SYNDICATION_LIVE`, no HubSpot, no Zillow POST.

## Why zip failed last night

Not because Neon is empty. `listListingSources` does not send photo bytes to the browser. Photos are `property_media ⋈ media.file_data`. The zip route must query that itself. Cap 25. Zip rows with bytes; if none, return a `.txt` of `/api/media/{id}` URLs. Gated `portal.read`. No CDN scrape.

## Block A — holes

1. Photo zip/txt route + Download photos button
2. Dashboard **Needs Lisa** across all listings (score < 70, stale, expired, off-market still live off-site)
3. ES chrome on Launch, Needs-me, off-market banner, Prepare helper (`No publica en Zillow`)

## Block B — 4–20

4. `?propertyId=` deep-link on the workbench
5. Stellar cannot go live without MLS#
6. Copy Matrix checklist (one clipboard)
7. Separate `#print-flyer` vs report print target
8. Last 5 inquiries on Launch
9. Withdraw Facebook+Clasificados ledger-only in one click
10. Regenerate stale only
11. Sighting rejects non-https and culebraluxe.com URLs
12. Constellation last event time
13. Sold price in just-sold blurb if a real column exists
14. Coming soon chip
15. Agent/office lock CulebraLuxe + source agent / env fallback
16. Expires-in-7-days count; pick expire-on-GET **or** cron, document once
17. Activity event stores price from→to on regenerate
18. Missing slug warning
19. Notes file: zip behavior + expire-once + commented vercel cron
20. Tests locking MLS# reject, off-market prepare, no-photo liveAttempt, dedupe, sighting reject, Needs Lisa helper, zip filename/url-list helper

At most migrations 104/105. Prefer zero.

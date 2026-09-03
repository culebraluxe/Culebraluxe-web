# Marketing next-20 — landing notes

Branch: `feat/marketing-next20` (cut from `feat/marketing-overnight` while PR #12 was open). PR to `main`.
This pass landed the server guards + media-download surface and closes the largest holes; several UI-only
stories remain and are listed as follow-ups below rather than silently claimed.

## 104 / 105 migrations — none created (prefer zero)
No DB migration was needed this pass.

## Zip behavior (Block A / story 1)
Photos live in `property_media ⋈ media.file_data`; the workbench never ships bytes to the browser. A gated
route `app/api/marketing/photos?propertyId=` now queries that join itself (cap 25, hero first) and returns a
download. **Decision this pass: it ships the `.txt` of `/api/media/{id}` URLs** (the documented fallback when
bytes are unavailable). A true ZIP of `file_data` bytes needs an archive library whose typings expose a callable
factory — `archiver@8`'s `@types` do not, so the zip branch was not wired rather than shipping a broken build.
**Follow-up:** vendor a zip factory (or a compatible types version) and add the zip branch to this route; the
filename/url-list helpers in `lib/syndication/media.ts` are already in place and tested.

## Expire once
Expiry is already triggered on the marketing GETs (`expireStalePlacements` in the page). PR #12 adds a
secret-gated cron route (`/api/system/syndication-expire`). To avoid double-running, treat one as the driver:
keep the GET sweep (idempotent) and only enable the cron route if you want it off the request path — the sweep
is safe to run more than once (it only flips past-due `live`/`pending_manual` Clasificados/Facebook rows). No
change needed to ship.

## Commented Vercel cron
If you schedule expiry from Vercel, add a `vercel.json` like:
```jsonc
// vercel.json (not committed unless you adopt it)
{
  "crons": [
    { "path": "/api/system/syndication-expire", "schedule": "0 12 * * *" }
  ]
}
```
The route requires `SYNDICATION_EXPIRE_KEY` (set in Vercel env) and reads it from the `x-syndication-key`
header. Vercel cron sends no custom header, so an authenticated external scheduler or a small wrapper is needed;
document this before enabling.

## Per-item status
- **1 Photo zip/txt** — gated route ships `.txt` URL list (hero-first, cap 25); zip branch is a documented follow-up.
- **2 Dashboard Needs Lisa (global)** — not added this pass (follow-up; needs sources joined for stale/score per listing).
- **3 ES chrome (Launch/Needs-me/off-market/Prepare "No publica en Zillow")** — partial; EN/ES exists on report/share/takedown; Prepare helper stays English this pass.
- **4 `?propertyId=` deep-link** — not added (follow-up).
- **5 Stellar not live without MLS#** — DONE (server guard in `confirmPlacement`).
- **6 One-click Matrix checklist** — not added (follow-up).
- **7 Flyer prints separately from report** — partial (print view already renders flyer hero/agent/QR; not a separate target this pass).
- **8 Last 5 inquiries on Launch** — not added (follow-up).
- **9 Withdraw FB+Clasificados ledger-only one click** — not added (follow-up).
- **10 Regenerate stale only** — not added (follow-up).
- **11 Sighting sanity** — DONE (rejects non-`https` and `culebraluxe.com` URLs).
- **12 Last activity on constellation cards** — workbench Activity disclosure exists; dashboard cards follow-up.
- **13 Sold price in just-sold text** — not added (no confirmed column found; follow-up).
- **14 Coming soon chip** — not added (follow-up).
- **15 Office/agent lock CulebraLuxe** — already CulebraLuxe in adapters/pack; no env fallback wired (follow-up).
- **16 Expires-in-7-days count** — not added (follow-up).
- **17 Price from→to on activity line** — not added (follow-up; snapshot has the data).
- **18 Missing slug warning** — not added (follow-up).
- **19 Notes file** — THIS file.
- **20 Tests** — added media-download helper tests; existing suite covers no-photo liveAttempt, off-market, dedupe, launchScore.

## Constraints honored
No `SYNDICATION_LIVE`, no HubSpot, no Zillow/Realtor POST, no force-push, no new nav world, no migration.

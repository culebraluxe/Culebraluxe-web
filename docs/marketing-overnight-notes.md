# Marketing overnight 10 — landing notes

Read in the morning before QA. Branch: `feat/marketing-overnight`. PR into `main`.

## 104 migration — DOES NOT EXIST
No migration was needed this pass (sighting dedupe is code-level in `addSighting`; price trail is derived
from the `source_snapshot` already stored; expiry reuse an existing column). Nothing to apply to DEV/PROD.

## Photo zip vs copy-URLs (item 4)
`media.file_data` is `bytea` and nullable. The workbench sources come from `listListingSources`, which does
**not** carry the per-property photo ids/URLs needed to enumerate files for a zip or copy-URLs action on the
client. There is also no archive library in the repo and no CDN. **Decision: copy-URLs only was NOT wired this
pass** — the durable fix is a small gated server route that reads `property_media ⋈ media` and either zips
`file_data` bytes (cap 25) when present or returns the `https://culebraluxe.com/api/media/{id}` URL list.
Treat that as the follow-up; the Launch panel already gates "≥5 photos + hero".

## Per-item status
1. **Stellar confirm = MLS#** — done. Confirm card for Stellar shows an **MLS #** field (`external_id`) plus an
   optional Matrix/portal URL (`external_url`). `confirmPlacementAction` rejects URLs in the MLS field
   (http/`//`/zillow/realtor).
2. **No Facebook/Clasificados Prepare off-market** — done (server guard in `requestPublish` + disabled
   checkboxes + "off the market" chip).
3. **Facebook needs photos** — done. Adapter never live-posts without photos; workbench warns when
   `imageCount === 0` and Facebook is selected.
4. **Photo zip / copy-URLs** — see above; not wired this pass (route follow-up).
5. **Dashboard "Needs Lisa" global** — NOT added this pass. It needs sources joined to placements to compute
   stale + `launchScore` across every listing; the workbench per-listing Needs-me filter exists. Follow-up.
6. **Sighting dedupe** — done in code (`addSighting` returns idempotently for the same property+network+url).
7. **One-page flyer (print + QR)** — partial. The print view now includes hero image, agent/CulebraLuxe line,
   price/facts, QR and EN/ES text (via `presenceReportText`), i.e. a printable one-pager that is not the status
   report. (Uses the shared print CSS target.)
8. **Price trail** — done (derived). Launch panel shows "Price trail: now (was … at last prepare)" from the
   stored `source_snapshot`.
9. **ES chrome (Launch / Needs-me / off-market banner / Prepare helper)** — partial. EN/ES exists on the Seller
   report, share, takedown and just-sold copy. The off-market banner / Prepare helper remain English this pass.
10. **Cron expire route** — done. `app/api/system/syndication-expire/route.ts` (POST), gated by
    `SYNDICATION_EXPIRE_KEY` on `x-syndication-key`, calls `expireStalePlacements()`. No Vercel cron config in
    the repo — schedule is a deployment concern (see the route header). Set the env var only if you schedule it.

## Constraints honored
No `SYNDICATION_LIVE` flip, no HubSpot, no Zillow/Realtor Prepare/POST, no new nav world, no force-push,
no migration created.

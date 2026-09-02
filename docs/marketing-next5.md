# Marketing next 5 — DeepSeek work order

Chris is QAing `feat/marketing-syndication` in **prod**. Do not force-push that branch. Do not flip `SYNDICATION_LIVE`. Do not merge to main unless he asks.

**Branch:** `feat/marketing-next5` cut from current `origin/feat/marketing-syndication`. PR back into `feat/marketing-syndication` when done.

No HubSpot. No Zillow/Realtor Prepare or POST. No new OperatingShell world. No public-site redesign.

## Task 1 — Printable seller report

Print CSS so only the report panel prints. Include listing QR on the print view. Days-on-market if published_at exists, else omit. Fix `presenceReportText` blank-line `|| true`.

Never say “Published to Zillow.”

## Task 2 — Dashboard constellation shows sightings

Load sightings on the dashboard. Chips: **Seen on Zillow** / **Seen on Realtor.com** / **Seen on Homes.com**. Click opens the pasted URL. No Prepare path.

## Task 3 — Off-market takedown kit

When listing is off-market and Facebook/Clasificados is live or pending: panel with copy ES/EN takedown text + checklist (Facebook, Clasificados, Matrix). Do not auto-delete third-party ads. Optional withdraw-ledger-only button labeled **Mark path withdrawn here**.

## Task 4 — Log inquiry from the portal (Slice E lite)

Launch panel **Log inquiry**: person + property_id + note + source phone/whatsapp/email/walkin. Reuse existing person/inquiry tables. New `listing_inquiry` only if none exists. Link Open client. No public form. No HubSpot.

## Task 5 — QR URL harden + Meta readiness copy

Allow apex, www, optional trailing slash on `/listings/{slug}` only. Reject portal URLs. Readiness line names missing env keys, never values. Keep **Live Graph POST is armed** when readyToPost.

## Order

1 → 2 → 3 → 4 → 5. Isolate migration for task 4 so 1–3 can merge first.

PR into `feat/marketing-syndication`. No force-push.

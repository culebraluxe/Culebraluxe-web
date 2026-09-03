# CulebraLuxe portal — next work orders

Marketing desk is feature-complete enough. Do not invent a seventh surface.

**Branch:** `feat/portal-hardening` from `origin/main` after marketing PRs #12/#13 land.

## Block M — last marketing code

**M1.** Canonical public URL is `/properties/{slug}` (what the live site uses). QR + blurbs + report must match. `/listings/{slug}` redirects there. QR allowlist: apex/www + `/properties/{slug}`.

**M2.** Public Enquire on `/properties/{slug}` writes `person` + `property_interest` (source `web`). Same path as Log inquiry. Honeypot + rate limit.

**M3.** Split `syndication-workbench.tsx` into launch / channels / packs / share / inquiry. No behavior change.

**M4.** Freeze further Marketing stories until Lisa uses Matrix + one Facebook paste.

## Block C — CORE

**C1.** Put `/portal/showings` in CORE nav or hide the orphan route.

**C2.** Cockpit one “needs a human” strip: catch-up + overdue deal actions + marketing needs-me + identity flags. Links only.

**C3.** Closed deal → marketing off-market banner. Do not auto-unpublish.

**C4.** Inspect Seller Strategy; reuse remarks in packs if they already exist.

## Block A — ACCOUNTING

**A1.** Closed deal → Create commission receivable. Not QuickBooks.

**A2.** Uncategorized receipts on the accounting dashboard if missing.

**A3.** No bank feed, payroll, QB sync. CSV for the CPA.

## Block O — OPS

**O1.** Identity Quality: merge two people if merge does not exist.

**O2.** Cannot publish a property without a slug.

**O3.** Media Audit: listings with no hero or <5 photos.

## Block S — SUPPORT

**S1.** Remove prod TEMP BYPASS on portal if still visible.

**S2.** Do not build a second Meta console.

**S3.** System Health: expire count + Facebook readyToPost (env names only).

## Block T — TECH

Do not expand Forge. Point Story Board at M1/M2.

## Block V — humans

Merge #12 then #13. Lisa: PRAR. Chris: MailerLite/Brevo DNS. WhatsApp broadcast lists in the app. Sleep.

## Token order

S1 → M1 → O2 → M2 → C1 → A1 → freeze.

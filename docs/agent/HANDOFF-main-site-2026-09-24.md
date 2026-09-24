# Handoff — main site fidelity, visitor tools, lead email (2026-09-24)

Work happens on `main` directly (owner's instruction: no side branches). Other streams push to `main` too
(security/entitlements, abstract service, OPPS property entry) — `git pull` / rebase before pushing.

## Shipped (all on main)

| commit | what |
| --- | --- |
| ff17cb7, a12af36 | Homepage cards: dead links fixed (Enquire → contact form with property, View All → /properties), interior SF, alt text; serif price, type eyebrow, highlights, scroll reveal (`.reveal` in app/globals.css) |
| ef84730 | Contact form back in Yew (`yew_views/contact.rs` → `/api/rust-ui/website-intake` → existing intake action); save hearts + `/favorites` page; hero/page-hero fade, CTA underline, FAQ accordion, mobile menu closes on nav, footer year |
| f02d81c | Buyers: Compare (max 3, side-by-side table), Saved searches (+N new alerts), live View filter. One matcher: `rust/ui/src/search.rs`. localStorage keys/shapes match the old TS |
| b447bb7 | Tagline: Rust `GET /v1/public/listing-copy` (authorized `property.public.read`), merged into the home payload |
| cd06a15 | Quick lead forms: "Send me these properties" (Saved page), "Tell me when new properties match" (Buyers, once a search is saved) |
| 431c6c5 | Mail sender: `rust/integrations/src/mail` (SMTP to iCloud, smtp.mail.me.com:587). `pnpm mail:test you@x.com` — VERIFIED working from Lisa's Mac |
| 199e2a0 | Lead emails: `POST /v1/website-intake/{id}/notify` → notice to lisa@culebraluxe.com (Reply goes to visitor) + visitor confirmation; once per lead via `notified_at` (migration 217). Policy: `website.lead.notify`, public-website actor only, reserved. `pnpm mail:preview you@x.com` sends samples |

## To make lead email live (owner actions)

1. `pnpm db:migrate` on PRODUCTION for `legacy/db/migrations/217_website_intake_notified_at.sql` (applied on the dev branch already). Prod actions are the owner's call.
2. Vercel env for BOTH the website and the Rust API: `ICLOUD_MAIL_ADDRESS=lisa@culebraluxe.com`, `ICLOUD_MAIL_USERNAME=lisa@culebraluxe.com`, `ICLOUD_SMTP_APP_PASSWORD=<app-specific password>`. Optional: `LEAD_NOTIFY_ADDRESS`, `PUBLIC_SITE_URL`.
3. Redeploy website + Rust API (the tagline read also needs the Rust API redeployed).
4. Run `pnpm mail:preview culebraluxe@gmail.com` once to see both emails.

## Decisions made with the owner

- All lead notices go to lisa@culebraluxe.com only.
- Site will have ~50 homes + 20 land, $0.5–4M — visitor tools are worth building.
- Visitor accounts: passwordless EMAIL CODE sign-in + existing Google. Sign in with Apple deferred (needs Apple Developer Program, $99/yr — owner asking partner; also needed for Apple Maps). Facebook Login optional later (Meta business verification already done for WhatsApp).
- Header "Saved" link stays off (earlier owner decision recorded in view.rs); Buyers page links to /favorites.

## Next (not started)

1. **Visitor sign-in (email code + Google)** as a separate external/visitor account type that can never reach the portal. Today non-staff sign-ins go to /login/unauthorized (identity resolution in Rust SecurityService). This is the SECURITY STREAM's area — coordinate before editing `rust/server/src/security/*`.
2. Sync saves / compare / saved searches to Neon per visitor (merge device localStorage on first sign-in).
3. Automatic saved-search alert emails (uses the mail sender).

## Environment notes

- The cloud sandbox cannot reach Neon over TCP or smtp.mail.me.com; the Neon MCP connector works for SQL. Live mail/DB tests run on Lisa's Mac.
- WASM UI artifacts are committed: after changing `rust/ui`, run `pnpm ui:build:release` and commit `lib/rust-ui/ui.js`, `lib/rust-ui/ui.d.ts`, `public/rust-ui/ui_bg.wasm`.
- `next dev` appends a Next.js block to AGENTS.md — revert it (`git checkout AGENTS.md`), don't commit it.
- Rust gates: `cargo fmt --all --check`, `cargo test -p ui -p server -p integrations`.
- SECURITY: the Neon password (shared by dev and prod), the Apple account password and the other keys in .env.local were pasted into a chat — rotate them.

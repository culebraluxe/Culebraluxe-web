# The Rust UI port — state of play

Read this before touching `rust/ui/**`, `lib/rust-ui/**`, `components/rust-ui/host.tsx`, or the two
`app/api/rust-ui/*` routes. It exists so a new session does not have to rediscover any of it.

## What the port is

The public site's pages are painted by Rust instead of React. The Next route is unchanged and still owns the URL; it
mounts `<RustUiHost start="site-…" pagePath="/api/rust-ui/public-page" />`, which boots a wasm module
(`rust/ui/**` → `lib/rust-ui/ui.js` + `public/rust-ui/ui_bg.wasm`) and hands it JSON. Rust renders markup; the host
owns the network, the session and the permission checks.

- `rust/ui/src/model.rs` — the screen registry, the model, the effects as types
- `rust/ui/src/update.rs` — messages to state and effects (`is_editorial()` lives here)
- `rust/ui/src/view.rs` — every renderer; pages are functions returning HTML strings
- `rust/ui/src/shell.rs` — the wasm entry points (`mount`, `rows_loaded`, `page_loaded`), the event listeners, and the
  repaint boundary: the whole document on mount or a screen change, `#rust-page` for everything else
- `rust/ui/src/icons.rs` — lucide icons as inline SVG
- `lib/rust-ui/boot.ts` — boots the module once, serves the effects (this file was gitignored until recently)
- `app/api/rust-ui/public-page/route.ts` — page payloads (blocks, listings, guide, property records)
- `app/api/rust-ui/public-rows/route.ts` — list payloads only; a page served as rows is a heading and three lines

## The rule that governs all of this

**No new TypeScript.** The job is converting the site OUT of TypeScript, so TypeScript is not a tool to reach for: not a
page handed back to a React body, not a new route, not a field borrowed "just for now". If a Rust renderer needs
something the payload does not carry, the control waits (trap 7) or the work stops and the question gets asked — never a
new TypeScript file. The TypeScript that remains — the routes, the two payload feeds, the host — is the scaffolding the
port is removing, not a surface to extend.

The one thing that will eventually have to change outside `rust/ui/**` is a server endpoint a browser form can POST to
(the Contact form). That is server code, not UI, and it is called out in Not done rather than written silently.

## Traps that have already cost days

1. **Read the page, not the component.** `components/*.tsx` are homepage *sections*. `app/<route>/page.tsx` is the
   page. Building `/services` from `components/services.tsx`, `/about` from `components/about.tsx` and `/buyers` from the
   marketing `buyers` block all produced the wrong page with the right-looking markup. The real pages are longer and have
   their own copy.
2. **`is_editorial()` and `custom_body()` must agree.** A screen with a renderer but no entry in `is_editorial` never
   fetches its page and renders nothing; an entry with no renderer fetches a payload nobody reads. `site-services` sat
   blank because of exactly this.
3. **The chrome belongs to the shell.** `render()` wraps every site screen with the header and the footer. A page that
   appends its own footer produces two — that happened to home, services and about.
4. **Every editorial route host needs `pagePath`.** Without it the screen asks for its page and is answered with an
   error; five of eight hosts were missing it.
5. **Never guess payload field names.** `PropertyDetail` has `_id`, `bedroomsTotal`, `bathroomsTotal`, `livingArea`,
   `lotSizeArea` — not `slug`, `bedrooms`, `bathrooms`, `lotSize`. The compiler catches it only because the payload is
   typed in TypeScript; the Rust side is permissive and would have rendered blanks.
6. **Icons are extracted, never remembered.** `waves` now draws `waves-horizontal` and `palmtree` draws `tree-palm`.
   Extract with:
   `node -e 'const R=require("react-dom/server"),L=require("lucide-react"),C=require("react");const s=R.renderToStaticMarkup(C.createElement(L["Compass"],{}));console.log(s.slice(s.indexOf(">")+1,s.lastIndexOf("</svg>")))'`
7. **The payload decides what a control can filter.** The Buyers bar can filter on what the payload carries and nothing
   else: the price arrives as a formatted string (`$2,400,000`) rather than a number, so `listing_price` reads the
   digits — and the view filter has no `views` field to read at all, so it renders disabled rather than pretending.
   Before wiring a control, check the payload has its field; adding one is a server change, not a Rust one.
8. **`view-dump --page` reads the payload from stdin.** Running it without a pipe waits forever, which is why several
   "hangs" were not hang. Correct form:
   `curl -s '<host>/api/rust-ui/public-page?screen=site-buyers' | cargo run -q -p ui --example view-dump -- site-buyers --page`
9. **The browser runs the WASM, not the source.** `cargo check` proves the source compiles and says nothing about what
   the page executes: a fix can be committed, pushed and invisible for days while the browser runs an artifact built
   before it. `pnpm ui:build:release` owns the artifact and the artifact is committed with the slice that changed it —
   `pnpm dev` now refuses to start when `rust/ui/src/**` is newer than `public/rust-ui/ui_bg.wasm`.
10. **Screen-local state must not destroy the chrome.** The shell repaints `#rust-page` and leaves the header and footer
    alone unless the model says the screen itself changed (`Program::chrome_signature`). Repainting the whole mount
    point for a keystroke rebuilt the navigation on every character — which closed the mobile menu, because a
    `<details>` open state belongs to the DOM and cannot survive being thrown away.
11. **`site_header` needs the model, because the header says where you are.** It used to `let _ = model` and so could
    mark no active destination; the stylesheet's `.top-nav-capsule[aria-current='page']` had nothing to match. One
    current destination per menu: the desktop capsules and the mobile capsules each mark the screen's own entry, the
    logo is marked on the home page, and a property record marks nothing (it is a child of Buyers, not a destination the
    menu offers).

## Ported, and how they were checked

| route | screen | payload | renderer |
|---|---|---|---|
| `/` | site-home | `public-page` (blocks + listings) | `site_home` |
| `/services` | site-services | none needed (literal copy) | `site_services` |
| `/about` | site-about | none needed (literal copy) | `site_about` |
| `/guide` | site-guide | `public-page` (guide catalogue) | `site_guide` |
| `/sellers` | site-sellers | none needed (literal copy) | `site_sellers` |
| `/buyers` | site-buyers | `public-page` (listings) | `site_buyers` + `buyer_showroom` (filters wired) |
| `/faq` | site-faq | `public-page` (hero + faq block) | `site_faq` |
| `/contact` | site-contact | `public-page` (hero + contact block) | `site_contact` (no form — see Not done) |
| `/properties/<slug>` | site-property-detail | `public-page` + `scope=<slug>` | `site_property_detail` |

Verify a payload, then the render, then look at the page — in that order, and never claim a page works from a green
build:

```
curl -s -o /tmp/p.json -w '%{http_code}' '<host>/api/rust-ui/public-page?screen=site-sellers'
cd rust && cargo run -q -p ui --example view-dump -- site-sellers --page < /tmp/p.json | head -c 400
```

## Not done

- **The Buyers view filter.** The bar's tabs, search, price, beds and sort are wired (the state is `Controls::named`,
  the messages are `QueryChanged`/`TabSelected`/`FilterSelected`, and the contract is `lib/search-contract.ts`, ported
  as `buyers_visible`). The view dropdown is disabled, because the payload carries no `views` field to match against —
  that is a server change, and until it lands the control says so rather than pretending.
- **Save / compare, the carousel's arrows, `SimilarProperties` and `RecentlyViewed`** on Buyers and on a property
  record. These are per-visitor reads (favourites, history) and a timed carousel; none of them is markup.
- **The Contact form.** `site_contact` renders the hero, the copy and the office and email, and no form: the live
  form's submission never completed in production, and doing it properly needs a POST endpoint a browser form can
  target. That endpoint is the one thing on this list that is server code, not Rust.
- **`scripts/ui-flip-readiness.mjs`** and the acceptance matrix from the brief (route → effect → endpoint → renderer →
  result). The runtime invariant that forbids "editorial screen → FetchPage → host without pagePath" is still
  outstanding — it is the trap that broke `/properties/<slug>`.
- The portal surface is untouched: it still renders its own React screens.

## Build and ship

```
pnpm ui:build:release     # builds the wasm + glue and reports their sizes
npx tsc --noEmit          # the host, the routes and the payload types
```

Commit and push to `main` — this repo has no other branches, and unpublished work is work nobody can review.

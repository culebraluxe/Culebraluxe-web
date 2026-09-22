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
- `rust/ui/src/shell.rs` — the wasm entry points (`mount`, `rows_loaded`, `page_loaded`) and the event listeners
- `rust/ui/src/icons.rs` — lucide icons as inline SVG
- `lib/rust-ui/boot.ts` — boots the module once, serves the effects (this file was gitignored until recently)
- `app/api/rust-ui/public-page/route.ts` — page payloads (blocks, listings, guide, property records)
- `app/api/rust-ui/public-rows/route.ts` — list payloads only; a page served as rows is a heading and three lines

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
7. **`view-dump --page` reads the payload from stdin.** Running it without a pipe waits forever, which is why several
   "hangs" were not hang. Correct form:
   `curl -s '<host>/api/rust-ui/public-page?screen=site-buyers' | cargo run -q -p ui --example view-dump -- site-buyers --page`

## Ported, and how they were checked

| route | screen | payload | renderer |
|---|---|---|---|
| `/` | site-home | `public-page` (blocks + listings) | `site_home` |
| `/services` | site-services | none needed (literal copy) | `site_services` |
| `/about` | site-about | none needed (literal copy) | `site_about` |
| `/guide` | site-guide | `public-page` (guide catalogue) | `site_guide` |
| `/sellers` | site-sellers | none needed (literal copy) | `site_sellers` |
| `/buyers` | site-buyers | `public-page` (listings) | `site_buyers` + `buyer_showroom` |
| `/properties/<slug>` | site-property-detail | `public-page` + `scope=<slug>` | `site_property_detail` |

Verify a payload, then the render, then look at the page — in that order, and never claim a page works from a green
build:

```
curl -s -o /tmp/p.json -w '%{http_code}' '<host>/api/rust-ui/public-page?screen=site-sellers'
cd rust && cargo run -q -p ui --example view-dump -- site-sellers --page < /tmp/p.json | head -c 400
```

## Not done

- **Interactivity.** The Buyers filter bar, the category tabs, save/compare, the carousel, the property tabs,
  `SimilarProperties` and `RecentlyViewed` all need state in the M4/MVI loop (`rust/ui/src/update.rs`) and a host that
  carries it. They are currently rendered as markup that does not act — deliberately, and labelled as such in the code.
- **`site-faq` and `site-contact`** have loaders and payloads but no renderers, so they show the NOT-PORTED notice.
- **The acceptance matrix** from the brief (route → effect → endpoint → renderer → result) and the runtime invariant
  that forbids "editorial screen → FetchPage → host without pagePath" are both still outstanding.
- The portal surface is untouched: it still renders its own React screens.

## Build and ship

```
pnpm ui:build:release     # builds the wasm + glue and reports their sizes
npx tsc --noEmit          # the host, the routes and the payload types
```

Commit and push to `main` — this repo has no other branches, and unpublished work is work nobody can review.

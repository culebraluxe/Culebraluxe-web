# UI Screen Architecture — the contract every screen implements

Status: **framework and master shell built** (owner decision 2026-09-26). Code: `rust/ui/src/app/`. Screens on the
trait: `db-test`, `site-account`. Cutover ledger: 19 screens still on the old loop (`app/registry.rs`). This document is the contract for all
UI work in `rust/ui`. It supersedes the ad hoc per-screen patterns: when code and this document disagree, the code is
wrong.

## Why

On 2026-09-26 the UI was one Yew MVI loop stretched across 83 screens:

- one `Model` (30 fields, each feature bolting its own state onto a shared struct), one `Msg` (179 variants), one
  `update.rs` (6,100 lines), one `Effect` (45 variants);
- generic fields (`rows`, `page`, `loading`, `error`) shared by every screen, so one screen's state leaks into the next;
- two renderers: 30 portal screens are Yew components, ~33 are HTML strings (`view.rs`) inside Yew, driven by a second
  event system (`data-*` attributes caught by document listeners in `shell.rs`);
- every screen doing loading, errors, fetching and navigation its own way;
- vendor islands wired by DOM scraping and `MutationObserver` races (1,320 lines for Projects alone);
- Next owning 85 page files, one per URL, each booting a fresh app with an empty model.

The recurring bugs are the inconsistency. The fix is one shape that every screen takes, with everything else owned by
the shell once.

## The shape

### 1. `Screen` — the abstract class

Rust has no inheritance; a trait with default methods is the equivalent. Every screen is one module implementing it.

```rust
pub trait Screen: 'static {
    // ---- the screen's own state: nothing global ----
    type Model: Default + Clone + PartialEq;
    type Msg: 'static;

    fn init(ctx: &ScreenCtx) -> (Self::Model, Cmd<Self::Msg>);
    fn update(model: &mut Self::Model, msg: Self::Msg, ctx: &ScreenCtx) -> Cmd<Self::Msg>;
    fn view(model: &Self::Model, ctx: &ScreenCtx, link: &Link<Self::Msg>) -> Html;
}
```

A screen's IDENTITY — key, path, surface, title, menu label, authority/entitlement — is its one line in the registry
(section 7), not trait constants: the router, both menus and the headless walk are generated from that table, and a
table is where "every screen" can be checked at once. (Subscriptions — timers, window events — are added as a named
capability when the first screen needs one.)

Rules:

- `update` is **pure**: no DOM, no `fetch`, no storage, no `spawn_local`, no `web_sys`. It changes the model and
  returns a `Cmd`. That is what makes every screen unit-testable without a browser.
- `view` is **pure**: model in, `Html` out, messages through `link`. No HTML strings, no `data-*` intents, no
  `Html::from_html_unchecked`.
- A screen owns only its own model. It never reads another screen's state. Shared facts (the actor, the URL's record
  id) come from `ScreenCtx`.

### 2. `ScreenCtx` — what the shell gives a screen (read-only)

```rust
pub struct ScreenCtx {
    pub actor: Actor,                 // who is signed in, level, entitlements (portal); None-equivalent on the site
    pub params: RouteParams,          // record id etc. from the route, typed
    pub query: QueryParams,
    pub now: Timestamp,
}
```

### 3. `Cmd<Msg>` — side effects as data, executed only by the shell

```rust
pub enum Cmd<Msg> {
    None,
    Batch(Vec<Cmd<Msg>>),
    Request(Request<Msg>),            // built from an Endpoint (below); the reply comes back as a Msg
    Navigate(Location),               // in-app navigation through the router
    Storage(StorageCmd<Msg>),         // device storage (favorites, compare, saved searches)
    Island(IslandCmd),                // push new props to a vendor widget
    Download(Download), Share(Share), // browser capabilities, each named, none generic
}
```

There is **no escape hatch** (no `Cmd::Custom(closure)`). A new capability is a new named variant, reviewed once,
executed in one place.

### 4. `Endpoint` — the typed API catalogue

```rust
pub trait Endpoint {
    const METHOD: Method;
    type Body: Serialize;
    type Response: DeserializeOwned;   // shared types from `rust/core/domain` where the server has them
    fn path(&self) -> String;
}
// update: Cmd::request(ListClients { search, page }, Msg::ClientsLoaded)
//   Msg::ClientsLoaded(Result<ClientsPage, ApiError>)
```

All endpoints live in `rust/ui/src/api/`. It is the **only** place that knows URLs. The shell's executor adds the
generation (so a reply to a screen you left is dropped), correlation id, error capture and decoding. When the HTTP
layer moves from the Next relays to Axum directly, the change is in this catalogue and the executor, and no screen
changes.

### 5. `Remote<T>` and the standard states

```rust
pub enum Remote<T> { NotAsked, Loading, Loaded(T), Failed(ApiError) }
```

A screen holding data holds a `Remote`. The shell's `remote_view(&remote, |data| ...)` draws loading and failure the
same way everywhere.

### 6. Base kinds — the abstract subclasses

Most screens are one of two shapes. They are generic building blocks a screen composes, so the behaviour exists once:

- **`ListScreen`** — search, filters, paging, row selection, open-record navigation, empty and error states.
- **`RecordScreen`** — load by id, edit draft, dirty tracking, validate, save, save-failed, leave-with-unsaved-changes.

A list screen supplies its row type, columns and endpoint; a record screen supplies its record type, form and
endpoints. Everything else is inherited.

### 7. `ScreenHost<S: Screen>` and the registry

`ScreenHost<S>` is one generic Yew component: it owns `S::Model`, runs `S::init`, feeds `S::Msg` through `S::update`,
executes the returned `Cmd` with the current generation, and draws `S::view` inside the chrome for `S::SURFACE`.

Screens are registered in one place:

```rust
// rust/ui/src/app/registry.rs — the table order is the menu order
entry("db-test", "/portal/db-test", Surface::Support, "DB Test", Menu::Rail("DB Test"), "portal.read", "portal.read",
      Kind::Screen(mount::<DbTest>)),
```

`Kind::Screen(mount::<S>)` is a screen on the trait. `Kind::LegacyPortal` / `Kind::LegacySite` host a screen still on
the old loop inside the new shell while it is ported — the ledger test (`legacy_count_only_goes_down`) holds that
number to a ceiling that only moves down. `Kind::External` is a page Next renders (Auth.js sign-in, the React Forms
editor). Adding a screen is **implement the trait + one registry line**. The registry's tests also check that every Next
`page.tsx` has an entry and every entry has a page, that the menus are the designed menus in order, and that every
surface's home is registered.

### 8. One app, one router

One Yew application serves every URL: the public site and the portal. The Yew router owns navigation, so moving between
portal screens is in-app, not a page load. Next is reduced to what only it can do:

- `app/portal/layout.tsx` — the server-side access guard (unchanged authority: the server decides);
- a catch-all page per area that renders `<RustUi />`;
- `/api/auth/*` (Auth.js) and, until the Axum cut, the relay routes.

Sign-in screens are Yew too. A form that posts to Auth.js must carry its CSRF token (see `yew_views/account.rs` for
the working pattern).

### 9. Vendor islands — dumb widgets

A vendor widget (SVAR Gantt, FullCalendar, Mux player) is rendered by one Yew component:

```rust
<Island kind="svar-gantt" props={json} on_event={link.callback(Msg::Gantt)} />
```

**Built:** `rust/ui/src/app/island.rs` (the component) and `components/rust-ui/island-host.tsx` + `island-renderers.tsx`
(the host and its kind → renderer table). First user: UI Lab.

Yew owns the node. A single small JS registry mounts the widget into that node, re-renders it when `props` change,
unmounts it when the node goes, and reports widget events back as typed messages. The widget holds no application
state, calls no API and reads no DOM outside its node. No bridges, no DOM scraping, no `MutationObserver`.

## What is forbidden, mechanically

Each of these is a CI gate (`pnpm gate:ui`), not a convention:

| Rule | Gate |
| --- | --- |
| No live TypeScript reaches `legacy/` | import-graph trace from every entry; baseline may only shrink |
| No screen state outside its own module | no new fields on a shared model; the old `Model`/`update.rs` may only shrink until deleted |
| No HTML strings, no `data-*` intents | `from_html_unchecked`, `render_page`, document listeners: zero outside the legacy adapter, then zero |
| No `fetch`/`web_sys`/`spawn_local` in a screen | only `shell/` and `api/` may use them |
| Relays stay thin | line budget per `app/api/**/route.ts`; orchestration belongs in Axum |
| Everything builds | `cargo test`, release wasm, `tsc`, `next build` |
| Every screen is reachable | headless walk of every registry path (`pnpm debug:portal-nav`, generalized) |

## Migration — complete, not partial

The owner's direction: done correctly, big-bang if needed; no permanent adapters.

1. **Framework** — `Screen`, `ScreenCtx`, `Cmd`, `Endpoint`, `Remote`, `ScreenHost`, `Island`, the registry macro,
   the executor, and the gates. Pilot: one simple screen and one list screen, end to end.
2. **Port every screen** onto the trait, by kind: lists, records, editorial site pages, bespoke workbenches (OPPS,
   TECH, Projects, Forms). Each port deletes its branch from the old `Model`, `Msg`, `update.rs`, `view.rs` and
   `yew_effects.rs`.
3. **Collapse Next** to the guard, the catch-alls and Auth.js.
4. **Delete** the old `Model`/`Msg`/`update.rs`/`view.rs`/`yew_effects.rs`, the `data-*` listeners, `StringBody`,
   the two mount paths, and the 85 page files. The gates then hold the line.

Done means: all gates green; zero screens outside the trait; the old loop deleted; every registry path walked headless.

## The screen inventory (owner-approved 2026-09-26)

This is the whole product. A screen not listed here is not ported: its route is deleted and its code goes with the
legacy tree. `app/registry.rs` holds exactly these routes, and its tests fail on any page or entry outside it.

**Public site** — Landing, Buyers, Property detail, Sellers, Services, Guide, About, FAQ, Contact; and, kept for
specific reasons: Properties (the homepage's "View All"), Favorites, Account (guest sign-in), Privacy (Meta requires a
public privacy page for WhatsApp Business messaging).

**Sign-in** — Login, Login recovery (emergency admin access), Unauthorized, Auth error. Sign-out is Auth.js's endpoint,
not a screen.

**Portal**

| World | Screens (drill-ins in brackets) |
| --- | --- |
| CORE | Cockpit [all activity, needs attention], Clients [client record], Projects [7 panes as tabs: Workplan, Timeline, Calendar, Financials, Documents, Activity, Catch-up], Contracts [contract record], Cabinet, Workflows [workflow record], Forms [form record], Seller Strategy — **ported** (Forms stays a Next page); Projects' five widgets are `<Island>`s |
| ACCOUNTING | Dashboard, Receivables, Expenses, P&L Statement, Receipt Scanner — **ported** (`app/screens/accounting/`: one shared model and reducer, five thin screens) |
| MARKETING | Dashboard, Syndication |
| OPPS | Records [property record], Listing Media |
| SUPPORT | System Health, DB Test, WhatsApp Diagnostic, WhatsApp Activation (a LIFELINE page — see below), WhatsApp Public Page (`/whatsapp`), Mux Video Test (`/video`), Security [users, roles, authorities]; plus the token review page (`/review/:token/:page`, public URL kept) |
| TECH | Cockpit, Flight Recorder (run detail), Storyboard [story record], UI Lab — **ported**; the sorter, the recorder console and the galleries are `<Island>`s |

Public URLs filed under SUPPORT keep their URLs (they may be registered with Meta or sent in email) and render in the
site chrome; the SUPPORT rail links to them.

**Retired (routes deleted 2026-09-26):** client-admin, decision-analysis, identity-quality, issues, media-admin,
needs-review, reporting, runtime-inspector, command-center, command-console (+ story), media-test, tech rust-lab,
app-errors, flight-recorder list, grok, kanban, lab, line, runs, both rust-previews, both dev map tests,
portal-auth-proof; and (same day) showings — its data source was unwired and returned nothing; bookings are the
Projects calendar — and the framer-ui-lab page, merged into UI Lab.

### Known gaps found while porting (not caused by the port)

- **`<select value>` lost its value in Yew**: Yew sets `value` before the options exist, so the browser showed the LAST
  option (Projects showed every project "Archived"). Fixed in every ported screen by marking the chosen
  `<option selected>`; the pattern is required for new screens. Old-loop screens (Forms, OPPS) still carry it until
  they are ported.

- **TECH sorter drop** still calls its Next server action (`moveStoryBucketAction`) from inside the island, as it did
  before the port. Everything else on the Cockpit is the screen's. Moving it behind an API endpoint the screen calls
  finishes §9 for this widget.
- **Story record** had no data source (it fell through to an empty generic rows read). It now reads the Cockpit's
  story detail (`/api/portal/rust-ui/tech?selected=<id>`).

- **Accounting** contract fixtures (`portal-page-accounting*.json`) are stand-ins written from the payload type in the
  cloud sandbox, which cannot reach the dev API; the next `scripts/ui-capture-fixtures.mjs` run replaces them. The old
  loop's accounting reducer arms and effects in `update.rs`/`yew_effects.rs` are now unreachable and go with the legacy
  tree. Fixed in the port: the Receipt Scanner no longer stays on "Creating…" after a save, and a refused P&L period
  keeps the statement on screen and says why.

- **System Health and Security** reads return `DATABASE` 500 from the Rust API on the dev database (2026-09-26). The
  screens show the template's failure state; their contract fixtures are marked stand-ins until a real capture works.
- **Authorities** shows the same role-entitlement rows as Roles — the old rows route answers both screens from one
  read. A real authorities read is a backend addition.
- **Mux Video Test** and **Review** have no rows source wired (the public rows route answers `[]`).

### Decisions recorded

- **WhatsApp Activation is a lifeline, not a cutover target.** The Meta Embedded Signup page
  (`app/portal/admin/whatsapp-coexistence/page.tsx`) is the exact TypeScript that activated WhatsApp Business messaging
  (restored from `706329da`, 2026-09-15, after a Yew conversion had dropped the launcher). Messaging works; the owner
  keeps this page as-is so there is a way back if activation must be redone, and changes it only deliberately. It is
  `Kind::External` in the registry. Do not convert it, test it against Meta, or "clean it up".
- **Activity and Needs attention** stay as drill-ins of the Cockpit, which links to them.
- **Forge's TECH Cockpit and Flight Recorder** were never properly ported and carry known bugs; they are fixed as they
  are ported, not before.

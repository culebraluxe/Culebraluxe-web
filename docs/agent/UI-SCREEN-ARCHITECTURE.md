# UI Screen Architecture — the contract every screen implements

Status: **framework, master shell, and every screen but two on the trait** (checked 2026-09-28). Code:
`rust/ui/src/app/`.

The registry (`app/registry.rs`) holds 58 entries: 54 `Kind::Screen` on the trait (CORE, ACCOUNTING, OPPS, SUPPORT,
TECH, and the whole public site), 2 still on the old loop — `marketing` (`/portal/marketing`, "Dashboard") and
`marketing-syndication` ("Syndication") — and 2 `Kind::External` (WhatsApp Activation, which the owner holds
deliberately, and `/portal` itself). The ledger test `legacy_count_only_goes_down` pins the legacy count at
`LEGACY_CEILING = 2`, and the ceiling only moves down.

**Next to convert: `marketing` — `/portal/marketing`, "Dashboard".** It is the Marketing surface's home path
(`navigation::home_path`), so the surface's front door is a legacy screen today, and it is the smaller of the two.
Neither screen has anything of its own to preserve: `view.rs::custom_body` has no arm for either, so both fall through
to the generic `rows()` table, and `rust/server` has no portal marketing read at all (`rust/server/src/marketing.rs` is
the *public* editorial content of the website, not this screen). Converting one is therefore a first build, not a port
— §7a's recipe from step 1. `marketing-syndication` follows, and it is a feature before it is a screen: its real
content is the Stellar listing draft (packet `MKT-STELLAR-DRAFT-01`) over schema 098/099/101/103, which no Rust code
reads yet.

**There is no Next.js application.** `rust/server/src/site.rs` answers every path with this one Yew app. What is left
of the old loop is Rust — `view.rs`, `update.rs`, `yew_effects.rs`, `yew_views/`, reached only through
`Kind::LegacyPortal` — and it is deleted when the last two screens leave it. TypeScript that is not this UI lives in
`legacy/` and is read-only.

This document is the contract for all UI work in `rust/ui`. It supersedes the ad hoc per-screen patterns: when code
and this document disagree, the code is wrong.

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
    pub actor: Actor,                       // who is signed in, as the portal layout handed it over; empty on the site
    pub id: Option<String>,                 // the record id a record route carries (`/portal/clients/:id`)
    pub query: BTreeMap<String, String>,    // the URL's query, decoded
    pub path: String,                       // the path this screen was opened at
    pub grants: Option<PortalEntitlements>, // what the user may do — UI VISIBILITY ONLY; the server authorizes
}
```

`ctx.can(action)` is the one place a screen asks "may I offer this?" — it mirrors the server's rule (internal
accounts only; managing entitlements and roles is ROOT's; nothing is offered before the grants arrive). A unit test
builds one with `ScreenCtx::default()`, which is why a screen can be tested with no browser and no session.

### 3. `Cmd<Msg>` — side effects as data, executed only by the shell

```rust
pub enum Cmd<Msg> {
    None,
    Batch(Vec<Cmd<Msg>>),
    Request(Request<Msg>),             // built from an Endpoint; method, path, body, and a reply that becomes a Msg
    Navigate(String),                  // in-app navigation through the router: no page load
    Load(String),                      // a full document load (site ↔ portal, or a server-owned route)
    ReplacePath(String),               // swap the URL without remounting: a workspace changing its record in place
    SharePdf { data_uri, filename, reply },   // the native share sheet, with a rendered PDF
    Listen { reply },                  // one utterance of the browser's speech recognition
    StorageRead { key, reply },        // one value from the device (favorites, compare, saved searches)
    StorageWrite { key, value },       // write, or remove with `None`
    After { millis, msg },             // deliver `msg` later: how a search waits for typing to pause
    Upload(Upload<Msg>),               // one file, chunked, so no request reaches the gateway's body limit
    VideoUpload(VideoUpload<Msg>),     // a property film, browser straight to Mux, with progress
}
```

There is **no escape hatch** (no `Cmd::Custom(closure)`). A new capability is a new named variant, reviewed once,
executed in one place: `app/exec.rs` is that place, and it is the only UI code allowed to touch `web_sys`.

### 4. `Endpoint` — the typed API catalogue

```rust
pub trait Endpoint {
    const METHOD: Method;                       // Get | Post | Put
    type Response: DeserializeOwned + 'static;  // shared types from `rust/core/domain` where the server has them
    fn path(&self) -> String;
    fn body(&self) -> Option<serde_json::Value> { None }
}
// update: Cmd::request(PortalScreenPage::of("db-test"), Msg::Loaded)
//   Msg::Loaded(Result<PortalPage, ApiError>)
```

The trait is in `app/cmd.rs`; the catalogue is `app/api.rs`, and it is the **only** place that knows a URL. The
executor (`app/exec.rs`) adds the correlation id, unwraps the `{ ok, value }` envelope and turns a refusal into a
typed `ApiError`. A screen never sees a URL, a status code or `fetch`.

What answers those URLs is the Rust server — the `/api/portal/rust-ui/**` and `/api/rust-ui/**` bridges in
`rust/server/src/api/portal_bridge.rs` and `routes.rs`. Adding a field to a read is a change to the server shape and
to `rust/ui/src/model.rs` together; there is no relay and no TypeScript in between any more.

### 5. `Remote<T>` and the standard states

```rust
pub enum Remote<T> { NotAsked, Loading, Loaded(T), Failed(ApiError) }
```

A screen holding data holds a `Remote` (`Default` is `NotAsked`, and the answer arrives as
`Remote::from_result(...)`). `template::remote(&model.read, "the database", |data| ...)` draws loading and failure the
same way everywhere — that is the whole point of the variant, and it is what a new screen should use rather than
writing its own spinner.

### 6. Base kinds — the abstract subclasses

Four shapes recur, and the three that are genuinely uniform already exist as blocks a screen composes:

- **`app/list.rs` — the list building block.** A list screen holds a `ListState` and wraps its messages
  (`Msg::List(ListMsg)`). `ListState::update` owns typing (with `SEARCH_PAUSE_MS` before the search runs), paging and
  saying when to reload; the screen owns which endpoint to call and how a row looks. The selection lives in the URL
  (`?selected=`), so it can be linked, reloaded and gone back to.
- **`app/rows.rs` — `RowsScreen<T>`.** A read that is genuinely just rows gets a whole screen from one generic
  component (`RowsScreen<Roles>`, used by the SUPPORT settings screens), selection and reload included.
- **`app/page.rs` — `PageScreen<T>`.** A portal screen that reads its page and draws it, with no control that changes
  anything. A `PageSpec` says which `screen=` it reads (`SCREEN`), what is being read (`NOUN`), whether the read is
  about the record in the URL (`SCOPED`), which part of the answer is its own (`pick` → `Option`, so a missing part is
  a failure to say so rather than an empty screen) and how it is drawn. Reading, loading, failure and the drill-in's
  back link are the module's, once — `Activity`, `Workflows`, `WorkflowRecord` and `Storyboard` are just specs. A
  screen that grows a command gets a module of its own.
- **Record screens** (client, deal, form, workflow, property, story, trace) have no shared trait: each is a `Screen`
  that loads its own read by id (`app/screens/deals/`, `clients/`, `forms/`, …). If a third one needs the same
  save–validate–dirty behaviour, that is the moment to extract it — not before.

Shared presentation is `app/template.rs`: `portal_heading`, `remote`, `loading_panel`, `failure`, `empty_panel`,
`widget_removed`, `metric`, `tabs`/`active_tab`, `back_link`, `with_query`, `input_value`. Use these before writing
markup of your own; a screen that draws its own loading state is the inconsistency this document exists to stop.

### 7. `ScreenHost<S: Screen>` and the registry

`ScreenHost<S>` (`app/host.rs`, 94 lines) is one generic Yew component: it owns `S::Model`, runs `S::init`, feeds
`S::Msg` through `S::update`, executes the returned `Cmd`, and draws `S::view`.

It also carries the **generation**, which is where stale answers die. Every message is stamped with it; a reopened
screen — new path, new record id, new actor — moves it on, and an answer to the previous record's request is dropped
there, once, centrally, so no screen has to remember to guard. Only the query changing (a `?tab=`, a `?selected=`)
keeps the state and asks `S::url_changed`.

Screens are registered in one place — 58 `entry(...)` lines, and the table order is the menu order:

```rust
// rust/ui/src/app/registry.rs
entry("db-test", "/portal/db-test", Surface::Support, "DB Test", Menu::Rail("DB Test"), "portal.read", "portal.read",
      Kind::Screen(mount::<DbTest>)),
```

- `Kind::Screen(mount::<S>)` — a screen on the trait. **The identity lives in the table, not in the trait**: key,
  path, surface, title, menu label and the two authority strings are the entry's columns, because the router, both
  menus, the breadcrumb and the walk are all generated from this one table.
- `.of("parent")` — a drill-in: it names the screen it hangs off (settings' users/roles/authorities, activity, every
  `:id` record route), so it is reachable and breadcrumbed without appearing in the rail.
- `Kind::LegacyPortal(key)` — the two Marketing screens still on the old loop. `legacy_count_only_goes_down` holds that
  number to `LEGACY_CEILING = 2`, and the ceiling only moves down.
- `Kind::External` — a route this app cannot render; today that is WhatsApp Activation, which draws a placeholder
  panel. See §8 for why it must not reload.

The registry's own tests are the gate on the table: keys and paths unique; paths resolve with their params, exact
segments winning; the rail is the designed menu in order; visibility follows the registry rules; a retired screen is
never in a menu; every drill-in names a real parent and highlights it; every surface's home is registered; and the
legacy count only goes down.

### 7a. Adding a screen — the recipe

`app/screens/db_test.rs` (174 lines) is the reference screen: one endpoint, one `Remote`, the template's states, one
test. Steps 1 and 2 are where a screen is actually designed; the rest is mechanical.

1. **The read, in Rust, first.** A screen that shows something needs a server answer. If a service method answers,
   reuse it; otherwise add the bridge arm in `rust/server/src/api/portal_bridge.rs` (the `match screen` in the page
   read / `support_payload`) and the shape in `rust/ui/src/model.rs` (for portal screens, a field on `PortalPage`).
   A screen never fetches and never knows a URL.
2. **The endpoint**, in `app/api.rs`: a struct and `impl Endpoint` (`const METHOD`, `type Response`, `fn path`, and
   `body()` if it writes). A screen that reads the whole portal page uses `PortalScreenPage::of("your-key")`, which is
   already there.
3. **The screen module**, `app/screens/<world>/<name>.rs`: `struct Name;`, a `Model` (`Default + Clone + PartialEq`),
   a `Msg` enum, and `impl Screen`. `init` returns the first state (start `Remote::Loading`) and
   `Cmd::request(endpoint, Msg::Loaded)`. `update` folds the answer in with `Remote::from_result`. `view` draws with
   `template::` helpers. Not fetching, spawning or reading storage in any of the three — that is the whole contract.
   **A screen with no command needs no module of its own**: hand it to `PageScreen<T>` (`app/page.rs`) or
   `RowsScreen<T>` (`app/rows.rs`) by writing the spec, and steps 3–6 shrink to the spec, the registry line and a test.
4. **One registry line**, and only one: `entry(key, path, surface, title, menu, authority, entitlement,
   Kind::Screen(mount::<Name>))`, or `.of("parent")` for a drill-in; `Menu::None` for a screen with no rail item.
   Never add a second mapping from path to screen.
5. **Declare the module** where its siblings are declared (`app/screens/mod.rs`, or the world's `mod.rs`, e.g.
   `accounting/mod.rs`) and bring the type into `registry.rs`'s imports — those two files are the only other ones that
   name a screen.
6. **A test in the screen's own file**, the way `db_test.rs` does it: assert the first `Cmd` carries the URL you
   meant (`cmd.into_requests().remove(0)`), feed the answer back through `update`, assert the model; then feed a
   failure and assert it says so. When a real payload exists, put it in `rust/ui/fixtures/` and decode it
   (`include_str!`), so the contract is checked against the server's actual shape.
7. **Verify**: `cargo check -p ui --features wasm --target wasm32-unknown-unknown --all-targets`. The whole `app`
   module and its tests sit behind `wasm` + `yew` + `yew-router` (`rust/ui/src/lib.rs:36-50`), so a plain
   `cargo test -p ui` compiles **no screen and no registry test at all** — it runs the legacy MVI's host-target tests
   and nothing else. Add `cargo check --workspace --all-targets` when the server shape changed too.

### 8. One app, one router

One Yew application serves every URL: the public site and the portal. `rust/server/src/site.rs` answers **every** path
with this app, so the Yew router owns navigation and moving between screens is in-app, not a page load. There is no
other page server and no relay layer: `app/`, `components/` and `lib/` are the retired TypeScript stack, out of scope
for the website (`docs/agent/LEGACY-TYPESCRIPT.md`). Access is decided server-side — a `/portal/**` request without a
resolved identity never reaches the app — and the app only draws what the grants it was handed allow.

That single fact is what `Kind::External` means in a running app. `Entry::needs_document()` is true for it, so
`chrome.rs::in_app` sends a link to or from such a route through the **document** rather than the router — and the
server answers that document load with this same app, which is how the WhatsApp Activation page once reloaded ~60 times
a second. An External route therefore renders a placeholder panel (`app/shell.rs:121-127`, `template::empty_panel`) and
is never a redirect or a `location` assignment. Two entries are External today: WhatsApp Activation (the Meta Embedded
Signup lifeline, held deliberately) and `/portal` itself.

`Cmd::Navigate` is an in-app move; `Cmd::ReplacePath` rewrites the URL without pushing history (a tab, a selection);
`Cmd::Load` is for a URL another server really owns (Auth.js's `/api/auth/**`), not a way to leave the app for a page
it could draw itself.

### 9. Vendor widgets — what "island" used to mean

**There is no `Island` component, no JS island host and no `MutationObserver` bridge in `app/`** (checked 2026-09-28;
the word "island" in this crate now means Culebra). The React/vendor widgets and the `components/rust-ui/island-*`
plumbing belonged to the Next world and went with it. What the widgets actually are:

- **Video** is an `<iframe>` on Mux (`https://player.mux.com/{playbackId}`) drawn by the screen that owns it
  (`app/screens/site/`, `app/screens/tech/`), and a film is uploaded straight from the browser to Mux through
  `Cmd::VideoUpload` (`app/exec.rs:54`), whose progress comes back as messages.
- **Timeline, calendar and gantt-style panes are this app's own drawing** (`rust/ui/src/timeline.rs`,
  `rust/ui/src/calendar.rs`) — Projects' Workplan, Timeline and Calendar panes are Yew, not SVAR.
- **Maps** are a Google Maps `<iframe>`.

If a vendor widget is ever genuinely needed, it arrives the way every other capability does: a named `Cmd` variant
owned by the executor (`app/exec.rs`, the only place a command touches the network, storage or the browser), with
messages back into the screen's `Msg`. The widget holds no application state, calls no API and reads no DOM outside its
own node — no bridges, no scraping, no observers.

## What is forbidden, mechanically

Each rule names the check that actually holds it today, so it can be enforced rather than remembered:

| Rule | How it is held |
| --- | --- |
| A screen does not reach the retired TypeScript | the Rust UI cannot load `legacy/` at all; on the TS side `pnpm lint` fails a new import and `eslint-suppressions.json` may only shrink; `pnpm broken:ts:sweep` keeps the dead-TS inventory honest |
| No screen state outside its own module | a screen's state is its own `Model`; the legacy global `Model` (`rust/ui/src/model.rs:2291`) and `update.rs` may only shrink until the last `LegacyPortal` screen is ported |
| No HTML strings, no `data-*` intents | `from_html_unchecked` has exactly two uses — `rust/ui/src/icons.rs:35` (the scoped SVG table) and `yew_portal.rs:32` (the legacy string body). `view::render_page`, `StringBody` and the document listeners in `rust/ui/src/shell.rs` are reachable only from the legacy loop, and go when it does |
| No transport in a screen | no `gloo_net`, no `fetch`, no `spawn_local` anywhere in `app/screens/`: a screen returns a `Cmd` and `app/exec.rs` performs it. `web_sys` there is allowed only for reading an event's target (input, select, drag, file) |
| One route table | `app/registry.rs` is the only mapping from path to screen; a second one is a review reject |
| Everything builds | `cargo check -p ui --features wasm --target wasm32-unknown-unknown --all-targets` (the only check that compiles `app/` and the registry — plain `cargo test -p ui` is the legacy MVI on the host target); `pnpm ui:build:release` (release wasm) and `pnpm build` (wasm + tailwind + the server binary); `cargo check --workspace --all-targets`. There is no Next build |
| Every screen is reachable | the registry's own tests, plus a headless walk of every registry path: `pnpm debug:portal-nav` |

## Migration — complete, not partial

The owner's direction: done correctly, big-bang if needed; no permanent adapters.

1. **Framework — in place.** `Screen`, `ScreenCtx`, `Cmd`, `Endpoint`, `Remote`, `ScreenHost`, `ListState` and
   `RowsScreen`, the executor and the registry are written and in use.
2. **Port every screen onto the trait — 54 of 58 entries.** The public site, sign-in, CORE, ACCOUNTING, OPPS, SUPPORT
   and TECH are ported; `marketing` and `marketing-syndication` are the two left on the old loop, held at
   `LEGACY_CEILING = 2`. Each port deletes its branch from the legacy `Model`, `Msg`, `update.rs`, `view.rs` and
   `yew_effects.rs`.
3. **Then delete the old loop**: when the last `LegacyPortal` entry goes, so do `Model`/`Msg`/`update.rs`/`view.rs`/
   `yew_effects.rs`/`yew_views/`, the document listeners in `shell.rs`, `StringBody`, `render_page`, and the raw-markup
   use in `yew_portal.rs`. (`icons.rs` stays: the ported screens use its SVG table.)
4. **Two entries are neither screen nor port**: the two `Kind::External` routes. Each is either ported or deleted —
   a permanent placeholder is not one of the two allowed outcomes.

Done means: every entry is `Kind::Screen`; the old loop is deleted; and every registry path is walked headless.


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
| CORE | Cockpit [all activity, needs attention], Clients [client record], Projects [7 panes as tabs: Workplan, Timeline, Calendar, Financials, Documents, Activity, Catch-up], Contracts [contract record], Cabinet, Workflows [workflow record], Forms [form record], Seller Strategy — **ported**, Forms and form records included (`registry.rs:161,197`); the panes, the timeline and the calendar are the screen's own (`app/screens/projects/`, `timeline.rs`, `calendar.rs`) |
| ACCOUNTING | Dashboard, Receivables, Expenses, P&L Statement, Receipt Scanner — **ported** (`app/screens/accounting/`: one shared model and reducer, five thin screens) |
| MARKETING | Dashboard, Syndication — **the two still on the old loop**; `/portal/marketing` is this surface's home path |
| OPPS | Records [property record], Listing Media — **ported**; the record route is the Workbench opened on that property; photos go through `Cmd::upload` (chunked) |
| SUPPORT | System Health, DB Test, WhatsApp Diagnostic, WhatsApp Activation (a LIFELINE page — see below), WhatsApp Public Page (`/whatsapp`), Mux Video Test (`/video`), Security [users, roles, authorities]; plus the token review page (`/review/:token/:page`, public URL kept) |
| TECH | Cockpit [Flight Recorder trace record], Story Board [story record], UI Lab — **ported**; the sorter is the screen's own (`Msg::SorterDropped`), and there is no island left on the Cockpit |

Public URLs filed under SUPPORT keep their URLs (they may be registered with Meta or sent in email) and render in the
site chrome; the SUPPORT rail links to them.

**Retired (routes deleted 2026-09-26):** client-admin, decision-analysis, identity-quality, issues, media-admin,
needs-review, reporting, runtime-inspector, command-center, command-console (+ story), media-test, tech rust-lab,
app-errors, flight-recorder list, grok, kanban, lab, line, runs, both rust-previews, both dev map tests,
portal-auth-proof; and (same day) showings — its data source was unwired and returned nothing; bookings are the
Projects calendar — and the framer-ui-lab page, merged into UI Lab.

### Known gaps found while porting (not caused by the port)

- **Public site ported** (2026-09-26): every public and sign-in page is a screen (`app/screens/site/`); the old site
  app (`yew_app.rs`, `yew_router.rs`) and the `LegacySite` kind are deleted. The interactive pages share one visitor
  model (`site/visitor.rs`: saved homes, compare, saved searches, recently viewed, enquiries, the gallery) with the
  TypeScript store keys unchanged.
- **Emergency sign-in had no form**: `/login/recovery` showed its two sentences and nothing to sign in with, so the
  break-glass provider in `auth.ts` was unreachable. It is a form again (posts to `/api/auth/callback/break-glass`).
  A refused credential is sent by Auth.js to `pages.signIn` (`/account`), not back here — `auth.ts` is the security
  stream's to change.
- **`/properties`** was a string-rendered rows table; it is now the collection as cards.
- **SEO**: the public site is rendered in the browser only (the Next page is an empty mount point; the Rust
  `document()` renderer is not served). Listings are invisible to crawlers that do not run JavaScript.

- **OPPS video upload** used to run inside an `opps-video` island posting straight to Mux. It is now
  `Cmd::VideoUpload`, performed by `app/exec.rs:54`, with progress and the answer coming back as messages
  (`screens/workbench/mod.rs`).
- **OPPS photo title** (`VillaDelMar_7`) was derived from the Listing Media payload, which the Workbench never loads,
  so it was always blank; it now comes from the Workbench's own property and photo count.
- **Listing Media** uploaded in one request (refused above the gateway's ~4.5 MB); it now uses the same chunked
  upload as the Workbench.

- **`<select value>` lost its value in Yew**: Yew sets `value` before the options exist, so the browser showed the LAST
  option (Projects showed every project "Archived"). Fixed in every ported screen by marking the chosen
  `<option selected>`; the pattern is required for new screens. Old-loop screens (Forms, OPPS) still carry it until
  they are ported.

- **TECH sorter drop** called a Next server action (`moveStoryBucketAction`) from inside an island. The island and the
  action are gone and the drop is the screen's own: `Msg::SorterDropped` sends `{"action":"moveStoryBucket"}` to
  `/api/portal/rust-ui/tech` (`screens/tech/mod.rs:200-218`). Its module doc comment still calls the sorter "the
  `tech-sorter` island"; that string exists nowhere else in the tree — stale prose, not stale code.
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

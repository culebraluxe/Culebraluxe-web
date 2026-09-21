# Layer: UI

**Owns:** what a person sees and does. Nothing else. No business rules, no SQL, ever.

## The state of the port (read this before assuming anything)

The UI conversion has two halves, and they are at very different stages. Saying "the UI is TypeScript" or "the UI is
Rust" is wrong in both directions:

| | state |
| --- | --- |
| **The screen model, navigation and registry** | **Ported to Rust.** `SCREENS` in `rust/ui/src/model.rs` is a table of **81 screens** — key, title, path, surface, nav, detail-of. The portal menu builds from it. |
| **The data feed for every portal screen** | **Wired.** `app/api/portal/rust-ui/rows/route.ts` serves the Rust model for every portal menu screen except `accounting-receipt-scanner`, from the same read models the TypeScript screens use. |
| **The host and the build** | **Done.** `components/rust-ui/host.tsx` mounts the WASM; `pnpm build` builds it (release, 221KB) and ships it. |
| **Screen-specific bodies** | **2 of 81.** `view.rs` dispatches a bespoke body for `tech-lab` and `projects`; everything else renders the model's *structure* — heading, navigation, rows — from generic cells. |
| **The cutover** | **Started: 10 of 81 screens.** `/portal/activity`, `/portal/attention`, `/portal/needs-review`, `/portal/workflows`, `/portal/identity-quality`, `/portal/media-admin`, `/portal/accounting/expenses`, `/portal/tech/flight-recorder`, `/portal/tech/runs`, `/portal/tech/line` render the Rust screen. `/portal/tech/lab`, `/portal/rust-preview` and `/rust-preview` mount the host too. The rest still render TypeScript. |

So the work that was done is real and substantial — the model, the registry, the navigation, the host, the WASM build
and the data feed all exist. **The flip is a per-screen job that had not started** and now has: ten read-only list
screens cross over, chosen because there is nothing in their TypeScript bodies the Rust screen cannot reproduce. What
remains is deciding the other screens one at a time.

## Which screens can flip, and why the rest cannot yet

Three conditions, and a screen needs all three. The first is a judgement; the other two are checkable:

| condition | how to check | screens failing it |
| --- | --- | --- |
| The Rust body carries the same information as the page it replaces | read `view.rs` and compare | 79 (only `tech-lab` and `projects` have a bespoke body) |
| The TypeScript body has no interaction to lose | count `useState`/`onSubmit`/`<form`/`onClick`/`Dialog`/`Modal` in the component the page renders | 15 of the 50 wired screens |
| The screen needs no record key | the host takes `rowsPath` and `start` only — there is no `scope` prop yet | every `[id]` route (`client-record`, `property-record`, `story-record`, `workflow-record`, `trace-record`) |

`scripts/ui-flip-readiness.mjs` computes the table from the code and prints the three groups. Current numbers:

- **81 screens.** 50 have a rows loader, 31 do not (those render structure only — `accounting-receipt-scanner` has no
  read model at all, by design).
- **35 are ready** — real rows plus a component with nothing interactive to lose. Eleven have flipped; the remaining 24
  are `accounting`, `accounting-pnl`, `accounting-receivables`, `db-test`, `forms`, `marketing`,
  `marketing-syndication`, `property-media`, `reporting`, `security`, `settings-authorities`, `settings-roles`,
  `settings-users`, `showings`, `storyboard`, `whatsapp-coexistence`, `whatsapp-meta`, `portal-root`, `dashboard`,
  `tech-app-errors`, plus the four record screens that also need the `scope` prop.
- **15 are blocked** by interaction their Rust body cannot reproduce — worst: `property-record` (74 markers),
  `property-admin` (31), `command-console` (29), `design-lab` (27), `projects` (17), `deals` (17), `issues` (12).

"Ready" means *nothing is lost*, not *nothing is different* — `dashboard` qualifies by the marker test but is the Cockpit
and has a layout of its own, so it is a judgement call rather than a mechanical flip. Read the two conditions
separately: the marker count tells you what breaks, it does not tell you what looks the same.

## Fronting a screen (the remaining mechanical step)

The host takes a screen key and a rows path:

```tsx
<RustUiHost rowsPath="/api/portal/rust-ui/rows" start="dashboard" />
```

A page becomes Rust by rendering that instead of the TypeScript component, using the `key` from the `SCREENS` table.
The comment on the page should say which screen it is and why it qualified — a flip is a decision, and the next person
needs to see the reasoning, not just the result.

It is per-screen rather than a switch for the reason above: a screen whose Rust body is still generic shows less than the
TypeScript one it replaced. The rows route is honest about that — a screen whose real columns have not been read shows
"Nothing to show yet" rather than an invented column.


## The Rust UI's shape (MVI)

One `Model` is the whole screen state, `Msg` is everything that can happen to it, `update` is the only place state
changes, `view` renders purely, `shell` mounts. Model 382 lines, view 726, update 225 — plus tests (`cargo test -p ui`).

## Building it

```bash
pnpm build               # the real build: Rust UI (release wasm) then Next
pnpm ui:build            # debug
pnpm ui:build:release    # release only
pnpm build:all           # local: workspace check, release server, tests, UI, Next
```

`scripts/rust-ui-build.sh` exists instead of `wasm-pack` for a documented reason (wasm-pack passes `--out-dir` to a cargo
that renamed it). The artifacts are **committed** — `lib/rust-ui/ui.js` (glue, imported) and
`public/rust-ui/ui_bg.wasm` (fetched at `/rust-ui/`) — because the frontend deploy does not run the Rust toolchain, and a
UI whose wasm is missing is broken at runtime with no build error. `scripts/build.mjs` rebuilds them when a wasm32 target
is present and keeps the committed ones when it is not.

## Rules

1. **No business logic in TypeScript.** Decide in Rust; render in TypeScript.
2. **No SQL in the UI layer.** The rows route calls the same read models as the TypeScript screens; it does not query.
3. **New screens are Rust.** A new screen gets a `SCREENS` row, a body in `view.rs`, and a page that mounts the host.

## Known gaps

- `app/api/portal/rust-ui/rows/route.ts` is **read-only and has no per-screen authority check yet** — stated in the route
  itself. Whoever gives it a write path must add authority first.
- 71 screens still render structurally rather than bespoke, and 15 of those that are otherwise wired carry interaction the
  Rust body cannot reproduce yet (the issue queue's filters and paging, the projects workspace's tree, Gantt and calendar).
- The host has no `scope` prop, so no `[id]` record screen can flip. Adding it is small and unblocks five screens at once.


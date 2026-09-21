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
| **The cutover** | **Not done.** The 82 files under `app/` still render the TypeScript components. The Rust UI is mounted on `/portal/tech/lab`, `/portal/rust-preview` and `/rust-preview`. |

So the work that was done is real and substantial — the model, the registry, the navigation, the host, the WASM build
and the data feed all exist. **What did not happen is the flip:** no route renders the Rust screen in the live portal, so
both UIs run side by side and the TypeScript components (144 of them) are still the live path.

## Fronting a screen (the remaining mechanical step)

The host takes a screen key and a rows path:

```tsx
<RustUiHost rowsPath="/api/portal/rust-ui/rows" start="dashboard" />
```

A page becomes Rust by rendering that instead of the TypeScript component — roughly one line per page, using the `key`
from the `SCREENS` table. It is deliberately per-screen rather than a switch, because a screen whose Rust body is still
generic would show less than the TypeScript one it replaced, and the rows route is honest about it: a screen whose real
columns have not been read yet shows "Nothing to show yet" rather than an invented column.

Order that works: flip a screen only when its Rust body can carry the same information as the TypeScript one. `projects`
and `tech-lab` can. The rest need their body first.

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
- 79 screens still render structurally rather than bespoke.

# Layer: UI

Crate: **`rust/ui`** — one Yew/WebAssembly app for the website and the portal, served by the Rust server
(`rust/server/src/site.rs`). **Owns:** what a person sees and does. Nothing else: no business rules, no SQL, ever.

There is no Next.js application and no TypeScript UI. `app/`, `components/`, the TypeScript host and the rows relay
were deleted in the port; `legacy/` is read-only reference (`docs/agent/BROKEN-TS-INVENTORY.md`).

The contract every screen implements, the holds, and the recipe for a new screen are
**`docs/agent/UI-SCREEN-ARCHITECTURE.md`**. This page is the one-screen summary.

## The shape (MVI, one trait)

| Piece | Where | Rule |
| --- | --- | --- |
| `Screen`, `ScreenCtx` | `rust/ui/src/app/screen.rs` | Each screen owns its `Model` and `Msg`; `update` and `view` are pure. The context carries actor, record id, query, path and grants (visibility only — the server authorizes) |
| `Cmd<Msg>` | `rust/ui/src/app/cmd.rs` | Side effects as data. No escape hatch: a new capability is a new named variant |
| Executor | `rust/ui/src/app/exec.rs` | The only UI code that touches `web_sys`: requests, navigation, uploads, storage, share |
| Endpoints | `rust/ui/src/app/api.rs` | The only place that knows a URL |
| Registry | `rust/ui/src/app/registry.rs` | The only path → screen map; router, menus and the headless walk are generated from it |
| Template | `rust/ui/src/app/template.rs` | How shared things are drawn |

The browser reaches `/api/portal/*` only (session cookie, `rust/server/src/api/portal_bridge.rs`); `/v1/*` is the
internal API. Sign-in is Rust too (`rust/server/src/api/google_auth.rs`, `ui_auth.rs`).

## Where the port stands (checked 2026-09-28)

`ENTRIES` in `rust/ui/src/app/registry.rs` has **59** lines: **57** `Kind::Screen`, **2** `Kind::External` (WhatsApp
Activation and `/portal`). The pre-trait global loop (`view.rs`, `update.rs`, `yew_effects.rs`, `yew_views/`,
`yew_portal.rs`, `document.rs`) was deleted on 2026-09-28; the public site's copy it held is in
`rust/ui/src/app/screens/site/content.rs`.

## Building and checking it

```bash
pnpm build        # the release wasm + Tailwind + the server binary
pnpm ui:check     # cargo check -p ui --features wasm --target wasm32-unknown-unknown — the pre-push gate
cargo test -p ui --features wasm   # the screen tests; without the feature they do not compile
```

A green `cargo check --workspace` does **not** prove the UI builds: the app is behind `--features wasm`
(`AGENTS.md`, 2026-09-28).

## Rules

1. **No business logic in the UI.** Decide in the server's services; render here.
2. **No SQL, no URL outside `api.rs`, no `web_sys` outside `exec.rs`.**
3. **Every screen is a `Screen` and one registry line.** A second map, a second renderer or an HTML string is a defect.
4. **Every request has a way out:** a timeout and retries; big uploads go in pieces (the gateway limit is about 4.5 MB,
   and Safari abandons long requests). The owner uses Safari; test in WebKit.

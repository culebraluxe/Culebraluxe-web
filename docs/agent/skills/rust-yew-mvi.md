# Skill: rust-yew-mvi

A screen is one folder under `rust/ui/src/app/screens/<screen>/` holding the parts of a single loop: `update.rs`
(the reducer) plus its view, document, fields and rail pieces. The reducer has exactly one shape:

```rust
pub(super) fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg>
```

- State changes happen in the reducer. The view reads `Model`; it never mutates it.
- I/O is a `Cmd`. `Cmd::request(FormsRead::record(id), Msg::RecordLoaded)` sends a typed request and names the
  message that carries the answer back. The request reports; the reducer decides what a failure means.
- A screen is registered with the entitlement codes it needs, e.g.
  `entry("cabinet", "/portal/documents", Surface::Core, "Cabinet", Menu::Rail("Cabinet"), "deal.read", "vault.read", Kind::Screen(mount::<Cabinet>))`.
  Adding a screen means adding its codes there, not only a route.
- `yew` 0.23 (`csr`) and `yew-router` 0.20 sit behind the `wasm` feature, which is ON by default so a plain
  `cargo test -p ui` and `cargo check --workspace` compile the real thing. The deploy's own check is
  `pnpm ui:check` = `cargo check -p ui --features wasm --target wasm32-unknown-unknown`.

## Anchored to
- `rust/ui/src/app/screens/forms/update.rs` — the reducer signature and `Cmd::request`.
- `rust/ui/src/app/registry.rs` — the screen registry and its entitlement codes.
- `rust/ui/Cargo.toml`, `package.json` — the features and `ui:check`.

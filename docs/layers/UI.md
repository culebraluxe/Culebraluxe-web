# Layer: UI

**Owns:** what a person sees and does. Nothing else. No business rules, no SQL, ever.

## Two UIs, on purpose

| | where | status |
| --- | --- | --- |
| The live screens | `app/`, `components/` — Next + TypeScript | **serving production** |
| The Rust screens | `rust/ui` — MVI, compiled to WASM | being ported screen by screen; internal screens only so far |

The Rust UI is mounted by `components/rust-ui/host.tsx`, currently on `/portal/tech/lab`, `/portal/rust-preview` and
`/rust-preview`. No client-facing screen uses it yet.

**Why both, and why they cannot disagree:** `app/api/portal/rust-ui/rows/route.ts` feeds the Rust model from *the same
read models the TypeScript screens call* — `@/db/clients`, `@/db/deals`, and so on. Two renderers, one source of data.

## The Rust UI's shape (MVI)

One `Model` is the whole screen state, `Msg` is everything that can happen to it, `update` is the only place state
changes, `view` renders purely, `shell` mounts. `SCREENS` in `rust/ui/src/model.rs` is a **table** — add a row to add a
screen.

## Building it

```bash
pnpm ui:build            # debug   → scripts/rust-ui-build.sh
pnpm ui:build:release    # release
```

The output (`lib/rust-ui/`, `public/rust-ui/`) is **gitignored** — it is a build artifact. That is deliberate, and it is
also the sharp edge: **the deploy does not run this script**, so a production build has no WASM unless someone builds it
first. See "Known gaps" below.

The script exists instead of `wasm-pack` for a documented reason — wasm-pack passes `--out-dir` to a cargo that renamed
it — and the script says to delete it when that is fixed.

## Rules

1. **No business logic in TypeScript.** Decide in Rust; render in TypeScript.
2. **No SQL in the UI layer.** The Next routes that feed the Rust model call the same read models as the TS screens;
   they do not query.
3. **New work in the Rust UI is MVI**: state in `Model`, transitions in `update`, rendering in `view`.

## Known gaps (do not discover these the hard way)

- `app/api/portal/rust-ui/rows/route.ts` is **read-only and has no per-screen authority check yet**. That is stated in
  the route itself. Whoever gives it a write path must add authority first.
- The WASM artifact is not produced by `next build`. Before any deploy that expects the Rust screens, run
  `pnpm ui:build:release` or wire it into the build, and decide whether the artifact should be committed.

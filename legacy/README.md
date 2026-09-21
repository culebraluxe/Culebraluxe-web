# legacy/ — the retired TypeScript server stack

```
legacy/db/            the data layer that used to be at db/
legacy/services/      the domain services that used to be at services/
legacy/workflow_app/  the TypeScript workflow engine that used to be at workflow_app/
```

Rust serves all of this now (`rust/`), and `docs/agent/LEGACY-TYPESCRIPT.md` is the migration plan. What is here is
kept because callers still route through it — the Next application under `app/` imports it — not because it is the
place to add anything.

## It is not the gate

**Do not treat this tree's test suite as the application's acceptance criterion.** It tests retired code. Running it
and driving it to green means spending days on TypeScript that Rust replaced, and it tells you nothing about whether
the live product works.

What says the product works:

```bash
npx tsc --noEmit     # types, whole repository — the move's own correctness
pnpm build           # the real build: Rust UI (wasm) then Next
cargo test           # the Rust workspace
```

Run the legacy suite only when the change you are making is *inside* `legacy/` — then run the file you touched:

```bash
node --import tsx --test legacy/workflow_app/tests/<file>.test.ts
```

It has 19 failing assertions of its own (13 of them were failing before this tree moved) and they are not a backlog to
work through. The ones worth knowing about are the guard tests that watch the *live* tree rather than this one —
`route-authority`, `db-boundary`, `no-tree-residue`, `rules-have-guards` — because those fail when the app changes
underneath them. Fix those when they fire; ignore the rest.

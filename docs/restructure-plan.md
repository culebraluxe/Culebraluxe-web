# Restructuring: where the code lives

Not to be done before a production deploy. This is the plan for after, so the reasoning is recorded rather than
re-derived.

## Where we are

The repository root is a Next.js application. The Rust workspace lives at `rust/` inside it. Nothing is wrong with that
mechanically — it builds, it tests, it serves — but it reads as though Rust is a subdirectory of a TypeScript project,
and the TypeScript that Rust replaced is still sitting in the same tree:

```
/                        <- package.json, next.config, app/, components/  (a Next app)
  rust/                  <- the domain, the API, the engine
  db/  services/  workflow_app/   <- the retired TypeScript server stack
```

An agent — or a person — arriving fresh has no structural signal about which tree is authoritative. AGENTS.md now says
so in words; structure should say it too.

## Where it should end up

```
apps/web/        the Next UI: app/, components/, public/, next.config
services/*.ts    the retired TypeScript server stack: db/, services/, workflow_app/
rust/            unchanged: the domain, the API, the engine
docs/            unchanged
```

Two signals fall out of that: Rust is a peer of the application rather than a subdirectory of it, and the retired
TypeScript is named as retired.

## Where it ended up

Step 2 is done, and the retired TypeScript moved to `legacy/` rather than to a top-level `services/`: the name says what
the tree *is* — retired — where `services/` would have read like a place to add things. The Rust workspace stays at
`rust/` and the app stays at the root for now.

```
/                        <- package.json, next.config, app/, components/  (the Next UI)
  rust/                  <- the domain, the API, the engine
  legacy/db/             <- the retired TypeScript server stack (was db/)
  legacy/services/       <- (was services/)
  legacy/workflow_app/   <- (was workflow_app/)
  docs/                  <- unchanged
```

What is left is step 3: moving the app itself under `apps/web/`, which changes the Next root, the `@/…` alias base and
the Vercel service root in one commit. It is the riskier half and it is not attempted yet.

## Why it is a project and not a move

Every one of these breaks something that must be fixed in the same commit:

1. **`next.config` and the `app/` router assume the project root.** Moving the app under `apps/web/` means a new root,
   a new `tsconfig` path base, and every `@/…` alias resolving from somewhere else.
2. **The TypeScript server stack is imported by the app** in both directions (routes call services; services import
   `db/`). Moving it without a codemod rewrites hundreds of import paths.
3. **Build and deploy configuration** — `package.json` scripts, the Vercel root, CI paths, `scripts/` assumptions,
   `docs/agent/` packet paths, and the `forge:packet-lint` manifest rows all name files by path.
4. **The packet lint fails on a stale path** by design, so every recorded evidence path in every story packet must be
   updated in the same pass — which is a feature: it is the map of what the move touched.

## Order, when it happens

1. Deploy Rust. Confirm a clean day in production.
2. **DONE — move the retired TypeScript** (`db/`, `services/`, `workflow_app/`) to `legacy/`, leaving the app where it
   is. The whole group moved in one commit because the three directories import each other: moving them together leaves
   every path inside the group untouched, so only the boundary had to be repaired. Verified with `npx tsc --noEmit`
   (clean) and `pnpm build` (clean). The suite that lives in the moved tree was **not** used as the gate — see
   `legacy/README.md` for why: it tests retired code, and it had failing assertions before the move as well.
3. Then the app (`app/`, `components/`, config) in its own commit, with the Vercel/CI root updated in the same commit.
4. Regenerate `docs/rust-parity-ledger.md` and re-run `forge:packet-lint`; the lint failing on old paths is the check
   that step 2 and 3 left nothing behind.

Nothing is deleted at any point. `legacy/typescript-server` keeps a working checkout of the TypeScript stack as it was
before this work began, for reference (see `docs/agent/LEGACY-TYPESCRIPT.md`).

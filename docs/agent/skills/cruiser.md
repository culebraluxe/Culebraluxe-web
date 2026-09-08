# Skill: cruiser (dependency-cruiser)

Enforce architecture boundaries, dependency rules, and cycles — the deterministic gate between layers.

## When to use
- **Assay / QA**, and **after** any structural change that moves files across layers (`db` / `lib` / `services` / `app`).
- Run: `pnpm exec depcruise --config .dependency-cruiser.js <dirs>` (e.g. `db lib services app agent-runtime`)

## Rules of thumb (already encoded in `.dependency-cruiser.js`)
- Repository boundaries own DB-driver value normalization; code above must not know driver types.
- Services must not reach into `db` internals; app must not call DB query executors directly.
- `no-circular` is hardened to `error` ⇒ a dependency cycle **HARD-FAILS** the gate (fail closed).

## Wiring status
Installed (npm dep `^18.2.0`). Gate is live via the config — a cycle fails CI/QA.

## Never
- "Fix" a cycle by routing around a layer (e.g. app calling the DB gateway directly) — that is the exact architecture drift the gate exists to catch.

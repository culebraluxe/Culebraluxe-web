# Preserved: Projects page BEFORE ChatGPT's Projects MVI

**Why this exists.** The Projects workspace (`/portal/projects`) was built and
iterated in this repo across `2f71c66` → `c2476ea` (MVI prototype → real WBS
wiring → real Project + WBS services → real person names). It was working.

ChatGPT's server pulled a copy of this repo (3 pm) and returned a "Projects MVI"
patch ~3 hours later. It was committed as `82a7038`. That patch **did not just
add a service layer** — it also **rewrote `app/portal/projects/page.tsx` and
layered ~278 lines onto `components/portal/projects-workspace.tsx`**, and in the
process it **deleted the defensive `try/catch` guards** around the identity
lookups that had kept the page from ever 500ing.

Removing those guards is what caused the live DEV failure:

```
[db:gateway] operation=select env=development kind=UNKNOWN code=22P02
ProjectsPage
```

`resolveIdentityNames` passed non-UUID text anchors (e.g.
`mvi2-fake-property-20260909`, `sea-to-soul`, `pr1`) into `where id = $1` on
`property.id` / `contract.id`, which are **`uuid`** columns → Postgres `22P02`
"invalid input syntax for type uuid" → whole page 500.

## Restore (exact, from the tag)

```sh
git tag -l 'preserve/*'                 # preserve/pre-projects-mvi  -> 8ddc9b0
git checkout preserve/pre-projects-mvi -- app/portal/projects/page.tsx
git checkout preserve/pre-projects-mvi -- components/portal/projects-workspace.tsx
```

## Plain-text copies (readable here, ignored by tsc/build)

- `page.tsx.txt` — pre-MVI page (`8ddc9b0`), **with** the original try/catch guards.
- `projects-workspace.tsx.txt` — pre-MVI workspace component (`8ddc9b0`).
- `service-projection.ts.txt` — pre-MVI projection (`c2476ea`).

## Current decision

We are **not reverting the MVI architecture** (services, playbooks, migration 141,
WBS→Project ownership). The review judged that work sound and aligned. Instead we
keep it and restore the missing resilience:

- `app/portal/projects/page.tsx`: UUID-guards on every identity lookup + per-lookup
  durable capture, so a bad/missing anchor degrades to an id fallback instead of
  taking down the page.
- `ui/projects/service-projection.ts`: null-`propertyId` documents no longer attach
  to projects that also have a null anchor.

Fix commit: `e14a576` (uuid guard, docs null-anchor) plus the resilience restore.

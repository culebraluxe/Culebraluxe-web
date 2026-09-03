# CORE-C1-SHOWINGS-NAV — Showings belongs in CORE nav

## Goal

`/portal/showings` appears in CORE (NEXUS) nav, or the orphan route redirects to dashboard. Lisa can find showings without leaving CORE.

## Scope

- `lib/navigation/registry.ts` NEXUS items
- `workflow_app/tests/navigation-registry.test.ts` snapshots
- This packet

Do not rebuild showings. Do not add a new operating surface.

## Architect brief

Inspect `app/portal/showings`. If it is a usable list, add `{ label: 'Showings', href: '/portal/showings', authority: 'portal.read' }` after Contracts. Registry is source of truth for snapshots. If the page is an empty dump, redirect to `/portal/dashboard` instead of adding nav.

## Context refs

- `docs/portal-next-work-orders.md` C1
- `lib/navigation/registry.ts`
- `workflow_app/tests/navigation-registry.test.ts`
- `app/portal/showings`

## Acceptance criteria

- `navigationForSurface('NEXUS')` includes Showings **or** `/portal/showings` redirects
- navigation-registry tests pass
- TECH/SUPPORT/OPS lists unchanged

## Preconditions

Showings route exists.

## Postconditions

No orphan CORE route.

## Skills

ui

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

- pnpm exec tsx --test workflow_app/tests/navigation-registry.test.ts
- git diff --check

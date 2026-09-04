# ENG-FORGE-V5-09 — Stale Work Recovery and Lease Semantics

## Goal
Make unattended Forge recover safely from crashed/stale workers without duplicate execution or manual database surgery.

## Depends on
ENG-FORGE-V5-03 accepted and published. This story is independent of V5-08; it may use the recovery primitives already present on current `main`.

## Scope
Define stale-running detection, bounded lease/heartbeat semantics, stale claim recovery, and single-worker protection using existing durable state. Do not add a new queue system.

## Acceptance criteria
1. A genuinely live worker is never reclaimed.
2. A stale Running/claimed item can be returned to an explicit recoverable state deterministically.
3. Recovery is idempotent and records why it occurred.
4. Max attempts and Hold/Error semantics remain authoritative for that work item without globally blocking unrelated eligible stories.
5. No second concurrent Smith is created for the same work item.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*recover*.test.ts agent-runtime/**/*work*.test.ts`

## Test mode
SCOPED only; use exact touched tests when available.

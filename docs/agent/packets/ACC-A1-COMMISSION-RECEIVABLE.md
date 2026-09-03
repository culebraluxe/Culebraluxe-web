# ACC-A1-COMMISSION-RECEIVABLE — Closed deal can create a commission receivable

## Goal

From a Closed deal, Lisa can create one accounting receivable tied to that deal/property. Not QuickBooks.

## Scope

- Inspect receivables writer + deal Closed stage
- Explicit **Create receivable** on a Closed deal using the existing receivables table
- Idempotent: second click does not duplicate
- This packet

No bank feed, splits engine, or auto-unpublish.

## Architect brief

Grep `receivable` / `commission` / `closed`. Default amount = deal/offer price if present; Lisa can edit. Description `Commission — {property name}`. Store `deal_id` if the column exists, else notes. Button only, never implicit on stage change.

## Context refs

- `docs/portal-next-work-orders.md` A1
- `docs/accounting/`
- `app/portal/accounting/receivables`
- `app/portal/deals`

## Acceptance criteria

- Closed deal shows Create commission receivable
- Insert visible on `/portal/accounting/receivables`
- Second click does not duplicate that deal
- Non-closed deal does not silently create a receivable
- P&L/expense pages still render

## Preconditions

Receivables list works. Closed stage exists.

## Postconditions

One Closed deal can be money owed in ACCOUNTING.

## Skills

ui
neon

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

- git diff --check

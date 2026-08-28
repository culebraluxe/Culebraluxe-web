# CulebraLuxe Startup Delivery Operating Rules

**Status:** Active operating rule  
**Effective:** 2026-08-27  
**Authority:** CTO/Product Owner

This file exists so a new agent/session does not reset CulebraLuxe into a Fortune-500-style delivery model or leave required production database work for a separate human step.

## 1. Delivery posture

CulebraLuxe is a startup and uses a rapid fix-forward delivery model.

Default posture:

- ship real functionality quickly;
- prefer one complete vertical slice over a chain of partial handoffs;
- DEV is for proving the change;
- Production is a normal release target and practical QA surface;
- use targeted tests/builds rather than broad ceremonial regression unless specifically warranted;
- avoid branch/PR/change-control ceremony unless explicitly requested;
- do not invent enterprise approval gates that the CTO/Product Owner did not ask for;
- if a bounded production defect appears, fix forward with the next release.

Architecture discipline still matters. Rapid delivery does not authorize duplicate models, unsafe destructive data operations, secret leakage, or bypassing canonical domain boundaries.

## 2. Database schema promotion is part of the same story

A story that changes, creates, or depends on database schema is **not complete** until the required schema is applied and verified in every environment the released application code will use.

For normal CulebraLuxe implementation work, Cline/Builder owns the complete database promotion cycle:

1. create or reuse the numbered migration under `db/migrations`;
2. apply and verify it in DEV;
3. run the story's targeted tests/build;
4. apply the same required migration(s) to PROD;
5. verify the required PROD tables, columns, constraints, indexes, views, and materialized views exist and match the code being released;
6. refresh derived materialized read models when the story requires current data;
7. only then report the story complete.

Do **not** return with:

- "migration ready for PROD";
- "DEV verified; PROD pending";
- "apply this later";
- a code commit that references schema not yet present in PROD.

A separate approval is not required merely because the story's required schema must be promoted to PROD. The implementation authorization includes the non-destructive schema promotion needed for that story.

## 3. Production data invariants

Schema promotion and application release work must preserve canonical business data.

Do not solve schema drift by:

- truncating canonical tables;
- copying DEV over PROD;
- resetting PROD;
- deleting clients/people/interactions to make a migration pass;
- rebuilding canonical history from scratch when a migration/read-model repair is sufficient.

Destructive business-data changes remain a separate decision and require explicit human authorization.

## 4. Migration/read-model completion rule

If released code references a new or changed database object, release completion requires proving that object exists in PROD before declaring success.

For materialized views, completion includes the applicable definition change plus refresh/verification when current data is required.

The practical invariant is:

> **Code + DEV schema + PROD schema + verification = done.**

Not:

> Code + migration file = done.

## 5. Environment routing

Vercel Production must never silently route to a DEV database.

Environment-selection code must fail closed on contradictory or missing production configuration rather than falling back from Production to DEV.

Explicit CLI/operator commands that select `dev` or `prod` may continue to do so, but the target must be unambiguous and secrets must never be logged.

## 6. Incoming-session rule

Every new CulebraLuxe agent/session must read:

1. `AGENTS.md`
2. `docs/ARCH-01-README-SUPPLEMENT.md`
3. this file
4. the current continuity packet / Story Board state relevant to the active work

Do not make the CTO/Product Owner re-teach this operating model after a context-window reset.

If a later explicit human decision conflicts with this file, the later human decision wins and this file should be updated.

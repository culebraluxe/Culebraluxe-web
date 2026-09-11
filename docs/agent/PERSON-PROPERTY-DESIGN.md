# PERSON ↔ PROPERTY — the simple design

Written 2026-09-10, after a long night. The goal: **simple enough that a 5-year-old gets it.**

## The picture — five lines

```
FEEDS  →  LANDING  →  PERSON / PROPERTY  →  VIEW
```

1. **FEEDS** — whatever a source gives us: Apple Contacts, iMessage, Gmail, forms, bank.
2. **LANDING (L tables)** — one L table per real table, 1-to-1 with it. The feed dumps straight in. Light cleaning only: trim spaces, split names, normalize phones. **No judgment, no merging.**
3. **PERSON** and **PROPERTY** — the two real tables. One row = one human. One row = one property.
4. **Everything else** is either a **join** (`person_property`, `person_firm`) or a **transaction** (`interaction`, `deal`, `offer`, `task`, `document`).
5. **VIEW** — the join the screens read, materialized for speed.

## The four rules

**R1 — ONE DIRECTION.** Feeds → Landing → Person/Property → View. Nothing copies backwards. Ever.
**R2 — NEWER WINS.** A later feed always beats an earlier one. Old data is deprecated, not merged.
**R3 — IDENTITY = PHONE or EMAIL.** That is how a landing row finds its Person. One normalizer, used on both sides, or the key drifts (we had `+40…` vs `+140…` for the same number).
**R4 — PROMOTE EVERYTHING.** If the landing row has it, Person gets it: name, phones, emails, addresses, note, org. Nothing is "name-only".

## Where we are, against this

| Piece | Today | Verdict |
|---|---|---|
| Feeds | Apple Contacts ✅, iMessage ✅, Gmail ✅, bank ✅ | keep |
| Landing for people | `l_person`, `l_person_identity`, `l_person_address` | **KEEP — this is the pattern and it works** |
| Landing for property | — none — | add when a property feed exists |
| Extra blob layer | `integration_staged_contact_profile` (JSON + revisions + fingerprints) | optional archive only; **not a second truth** |
| Person | `person` + `person_identity` | **KEEP** (21 tables point at it) |
| Property | `property` | **KEEP** |
| Joins | `person_property`, `person_firm`, `person_person`, `property_interest` | keep |
| Business / roles | `firm` (new, yours) | **KEEP** |
| View | `mv_client_directory` (+ 2 others) | keep, collapse to what we actually read |
| Unused columns | `property.legal_owner_name`, `property.seller_person_id` | your call |

## The fix — three slices, in this order

**S1 (hours) — promote EVERYTHING, newest wins.**
`l_person` → `person` currently carries **only the name**. Phones, emails, addresses and note never cross. That single gap is why the ODS has been right for weeks while Person stays empty. This is the "doesn't work for 5 weeks" bug.

**S2 (days) — make the flow one-directional.**
Screens read Person / Property / View. Delete every step that copies a fact into a second place.

**S3 (days) — one view.**
Fold the extra MVs into the one the screens need.

## What NOT to do
- **No new fact tables.** If a fact is in Landing, Person shows it *through the view* — never store it twice.
- **No second identity system.** Phone/email, one normalizer, used everywhere.
- **No merge logic in feeds.** Merging happens once, in Person.
- **No facts stored on Person that belong to a source.** Person holds identity; Landing holds facts.

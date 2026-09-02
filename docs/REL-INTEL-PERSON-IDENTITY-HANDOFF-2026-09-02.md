# REL-INTEL PERSON / IDENTITY / RELATIONSHIP ROLLUP HANDOFF

**Date:** 2026-09-02  
**Repository:** `culebraluxe/Culebraluxe-web`  
**Environment under investigation:** PRODUCTION  
**Neon project:** `CulebraluxeData`  
**Database:** `neondb`  
**Production branch:** `br-snowy-fog-axg3jae2`  
**Current main before this handoff document:** `b53f89151820d984c035ec246587622e06b7c602`

---

## 1. Why this document exists

The current ChatGPT session has been acting as engineering copilot on a long-running relationship-intelligence / identity-mastering problem. The Neon plugin repeatedly becomes unusable because its `run_sql` wrapper advertises camelCase arguments while the backend rejects them and demands snake_case. When that happens, the only practical recovery has been to remove/re-add the plugin and start a new chat, which loses investigative continuity.

This file is the complete handoff for the next engineer / agent. Read this before changing identity, relationship, Apple, Gmail, call, FaceTime, or Client Contact History code.

The goal is **not** to redesign the model. The current Person-first model is the intended architecture. The remaining work is to prove where existing source identities and communication evidence fail to converge onto the same canonical Person, then fix only that seam.

---

# 2. Binding architecture: Person is the driver

The canonical model is:

```text
Person
  -> 0..n phone identities
  -> 0..n email identities
  -> 0..n Apple/source handles
  -> 0..n WhatsApp/source identities
  -> communications from all paths roll up to that Person
```

A phone number is **not** the Person. An email address is **not** the Person. A provider/source identity is **not** the Person.

The canonical relationship rule is:

> For a Person, relationship activity is the union of communication evidence associated with every active identity/source handle owned by that Person.

The read model should remain **Person x Source**, not per phone and not per message.

Do not solve this by choosing a primary phone/email as the relationship join key.

---

# 3. Current schema / implementation pieces that already support the correct design

## 3.1 `person` + `person_identity`

`person` is the canonical human/client/transaction participant.

`person_identity` stores 1-to-many communication identities for the Person. Current mastering supports multiple emails and phones on one Person.

Important implementation:

- `db/person-identities.ts`
- `db/person-mastering.ts`

`semanticPhoneKey()` normalizes NANP numbers so a 10-digit Apple value and E.164 `+1...` are treated as the same exact identity.

## 3.2 Durable source-specific ownership

Migration `097_source_person_link.sql` created:

```text
integration_source_person_link
```

This table answers one question only:

> Which canonical Person owns this source identity?

It is source-neutral and is the durable source identity -> Person ownership seam.

## 3.3 Relationship evidence

`integration_relationship_evidence` is the current source-grain evidence store.

It contains:

- source / source_account / source_identity_key
- emails / phones / display name / organization
- first/last observed
- last inbound/outbound
- inbound/outbound counts
- two-way flag
- canonical_person_id
- review_state
- match method / reason / confidence

It is evidence/context; it is not the canonical Person itself.

## 3.4 Relationship read model is already Person-first

Migration `094_mv_client_relationship_channels.sql` groups evidence by:

```text
canonical_person_id, source
```

Multiple source handles for the same Person + source reduce into one row.

Therefore the main defect is **not believed to be the MV grouping logic**.

If a second Apple handle is exact-linked to the same Person, the MV should already aggregate it correctly.

The leading defect is upstream ownership/reconciliation of identities/source handles, or absent source ingestion.

---

# 4. Important historical symptom

The Client Contact History UI showed aggregate relationship counts but missing source rows / fragmented people.

The architecture has gone through multiple loaders / cleanup passes. User estimates roughly eight loader generations and many identity fixes.

The user expected the latest model/reload to migrate all 1-to-many children to the correct surviving Person.

That expectation is reasonable; however, current production evidence shows some paths still do not converge.

---

# 5. Primary regression case: Alicia

**Alicia is the best fixture because she exercises more paths than Ami.**

Historically Alicia has had evidence from multiple channels and previously exposed bugs in multi-channel relationship logic.

Known / visible production state from the Client UI on 2026-09-02:

Selected canonical card:

```text
Alicia Geigel
email: alicia.geigel@gmail.com
phone: none shown
observed communications: 11
```

Contact History on that selected Person showed approximately:

```text
iMessage: 1
Gmail: 10
Phone: no activity connected
FaceTime: no activity connected
```

The search results on the left also visibly included other Alicia-like rows, including:

```text
alicia
phone: +1 727-420-1806
about 42 observed
```

and another row associated with:

```text
alicia.geigel@yahoo.com
```

Do **not** assume from UI alone that these are definitely mergeable duplicates. The current cleanup attempt specifically designed for one obvious legacy split pattern found zero eligible merges. The actual DB graph must be traced.

Expected eventual state for Alicia:

```text
ONE canonical Person
  - Gmail address(es)
  - Yahoo address if it belongs to the same Apple/source profile
  - phone(s)
  - Apple Messages handles
  - Apple call handles
  - FaceTime handles

Separate Person x Source relationship rows, all on the same person_id.
No duplicate Person just because a new communication path appears.
```

---

# 6. Secondary regression case: Ami

Ami is useful for the multiple-phone / Apple-handle case.

Known production UI state before the call-history work:

```text
Ami
phone: +1 860-989-5020
email: none
4,858 observed
2,434 inbound
2,424 outbound
two-way
iMessage connected
PHONE blank
FACETIME blank
```

User says Ami and Lisa call each other frequently, so call/FaceTime history is expected to exist somewhere.

This led to the Apple CallHistory investigation described below.

---

# 7. Fixes already made during this session

## 7.1 Semantic phone normalization in in-memory reconciliation

A real mismatch existed:

- canonical matcher already treated `+1 787...` and `787...` as the same NANP identity
- two high-volume caches did not

Changes made:

- `lib/relationship-intel/inmemory-lookup.ts`
- `db/person-mastering.ts`

Both now use the same NANP semantic phone key behavior.

Alicia-style tests were added for multiple phones + email + multiple Apple handles + Gmail converging onto one Person.

Relevant earlier commit:

```text
9454ce7b0ff85c844877af48e7f6b49613e3d759
```

## 7.2 Apple CallHistory / FaceTime ingestion added

The existing Apple Messages exporter reads only:

```text
~/Library/Messages/chat.db
```

That DB contains messages/handles but not normal call-history events.

A new read-only Apple CallHistory path was added using:

```text
~/Library/Application Support/CallHistoryDB/CallHistory.storedata
```

New files included:

- `scripts/macbridge/AppleCallHistory.swift`
- `lib/relationship-intel/apple-calls.ts`
- `scripts/apple-calls-intake.ts`
- `scripts/apple-calls-sync.sh`

Initial Swift compile issue was fixed by splitting a large nested `[String: Any]` expression.

The real Mac run succeeded with:

```text
CALL HISTORY EXPORT SUCCESS
rows=4980

APPLE CALL HISTORY INTAKE
calls: 4980
normal calls: 4880
facetime calls: 100
evidence rows built: 827
reconcile tally:
  exact_linked: 545
  unmatched: 111
  review_required: 156
  deferred: 15
interactions inserted: 4510
interactions replayed: 0
skipped unlinked: 470
skipped no date: 0
skipped no address: 0
errors: 0
```

This proves:

1. Apple CallHistory is accessible on this Mac.
2. There are real FaceTime records: **100** in this dataset.
3. The exporter and intake work end-to-end.
4. The remaining interesting set is the unlinked/review/deferred identity group, not extraction itself.

Important current semantic detail:

- normal calls use source like `apple_calls`
- FaceTime uses source like `apple_facetime`
- canonical `interaction.channel` still uses `call`
- UI source matching can distinguish FaceTime by source name

Do not delete the PHONE/FACETIME slots from the Client Contact History UI. The user explicitly wants those positions retained.

---

# 8. Existing production cleanup that was inspected

`normalize-phone-identities.ts` already attempts to fix NANP duplicates and cross-Person phone collisions.

Important behavior found in that script:

1. It chooses a winner Person for a conflicting NANP group.
2. Deletes loser phone identity rows.
3. Moves remaining `person_identity` children to the winner.
4. Iterates all FKs referencing `person` and updates loser -> winner.
5. Archives the loser Person.

However, the generic FK migration has this behavior:

```text
exception when unique_violation then
  null;
```

That means a uniqueness collision can be swallowed while the rest of the merge continues.

This is dangerous because it can leave some child row stranded on an archived/loser Person or otherwise produce an incomplete consolidation.

Do not repeat this pattern in new repair logic.

Any future Person consolidation must be transactionally all-or-nothing for child ownership.

---

# 9. Why normal Person mastering does not repair legacy split Persons

Current `db/person-mastering.ts` is conservative by design.

For a current source profile:

- if durable `integration_source_person_link` already exists, that Person is authoritative for that source identity
- mastering tries to attach the profile's identities to that Person
- if one of those identities is already owned by another Person, `attachSafeIdentities()` returns conflict
- result is ambiguous / conflict instead of silently merging two canonical Persons

This is correct behavior for normal ingestion.

It also means a legacy split created by an older loader does **not** automatically heal merely because the new source profile contains both identities.

A separate, explicit consolidation process is needed when deterministic evidence proves that the legacy fragment belongs to the source-linked Person.

---

# 10. New conservative Person split repair added during this session

Files added:

```text
lib/relationship-intel/person-consolidation-plan.ts
workflow_app/tests/person-consolidation-plan.test.ts
scripts/repair-source-person-splits.ts
```

Package commands added:

```text
pnpm run identity:source-split:audit:prod
pnpm run identity:source-split:cleanup:prod
```

The repair algorithm is intentionally conservative:

1. Start from an already-established authoritative `apple_contacts` source link.
2. Build the Apple Contact profile's complete email/phone identity set.
3. Find other active Persons that own one of those identities.
4. A loser Person is eligible only when **every identity owned by that loser is contained in the same authoritative Apple Contact profile**.
5. If one loser is proposed for multiple different survivors, skip it.
6. Move/dedupe `person_identity` children.
7. Move all single-column Person FK children transactionally.
8. If any uniqueness conflict is encountered, abort the entire transaction; do not swallow it.
9. Archive loser only after child movement succeeds.
10. Refresh Client read models.

Tests cover:

- Alicia-style Gmail Person + phone Person + Yahoo Person -> one authoritative Person
- candidate with extra identity outside source profile -> no auto-merge
- candidate proposed for two different survivors -> no auto-merge

Relevant commits in order:

```text
8272f9d6f18a30e527e4186dbd3fc82c5b461bba
2864789f3591ac7a3b740ba9284b85bc7c94c8d5
294c796ac6d5502d1c27dbc1e316344ee016aa3b
c8f002c38693c1287fd7a23c0b0fd2578c6f6ac5
b53f89151820d984c035ec246587622e06b7c602
```

One accidental package-version drift happened while adding package scripts (`react-arborist` changed to `^3.14.10`), causing Vercel frozen-lockfile failure. It was immediately restored to the repository's existing `^3.16.0` in `b53f891...`.

---

# 11. Result of the production consolidation attempt

The user ran:

```bash
pnpm run identity:source-split:cleanup:prod
```

Production target was correctly resolved:

```json
{
  "target": "prod",
  "appEnv": "production",
  "neonBranch": "ep-flat-art-ax92tn7a-pooler.c-4.us-east-2.aws.neon.tech"
}
```

Result:

```json
{
  "source": "apple_contacts",
  "eligibleConsolidations": [],
  "skippedMultiWinnerLosers": [],
  "skippedPartialIdentityLosers": [],
  "mergedPersons": 0
}
```

Interpretation:

- Script ran correctly.
- It made no changes.
- Production did **not** contain the exact safe legacy split pattern this script was designed to repair.
- Do not broaden the auto-merge heuristic just to force Alicia to merge. First inspect the real graph.

---

# 12. Neon plugin blocker

Direct Neon read-only investigation is currently blocked by a connector/schema defect.

The tool advertises arguments like:

```text
projectId
branchId
databaseName
```

but the backend rejects them and says:

```text
project_id is required
```

and also rejects the camelCase keys as unrecognized.

This has repeatedly happened across chats. It is a plugin/wrapper issue, not evidence that the DB permissions are missing.

The user has been deleting/re-adding the Neon plugin and starting a fresh chat as a workaround.

When Neon access works again, the **first task is read-only graph discovery**, not another cleanup.

---

# 13. Exact next investigation to run when Neon works

## 13.1 Locate Alicia and Ami canonical Person rows

Start broad because display names may differ in case/surname:

```sql
select id, display_name, role, status, archived_at, created_at, updated_at
from person
where lower(coalesce(display_name,'')) like '%alicia%'
   or lower(coalesce(display_name,'')) like '%ami%'
order by display_name, archived_at nulls first, created_at;
```

Do not assume every returned Alicia/Ami row is the same person.

## 13.2 Enumerate every canonical identity for those Persons

```sql
select
  pi.id,
  pi.person_id,
  p.display_name,
  p.archived_at,
  pi.identity_type,
  pi.identity_value,
  pi.source_system,
  pi.is_primary,
  pi.created_at,
  pi.updated_at
from person_identity pi
join person p on p.id = pi.person_id
where pi.person_id in (<candidate person ids>)
order by p.display_name, pi.person_id, pi.identity_type, pi.is_primary desc, pi.identity_value;
```

Pay special attention to:

```text
Alicia:
  alicia.geigel@gmail.com
  alicia.geigel@yahoo.com
  +1 727-420-1806 / semantic equivalent

Ami:
  +1 860-989-5020
  any second phone
```

## 13.3 Enumerate durable source ownership

```sql
select
  source,
  source_account,
  source_identity_key,
  canonical_person_id,
  link_method,
  link_reason,
  linked_at,
  updated_at
from integration_source_person_link
where canonical_person_id in (<candidate person ids>)
order by canonical_person_id, source, source_identity_key;
```

Also search by candidate phones/emails/source keys even if their canonical_person_id is another Person.

## 13.4 Enumerate relationship evidence

```sql
select
  source,
  source_account,
  source_identity_key,
  display_name,
  emails,
  phones,
  first_observed_at,
  last_observed_at,
  last_inbound_at,
  last_outbound_at,
  inbound_count,
  outbound_count,
  is_two_way,
  canonical_person_id,
  review_state,
  match_method,
  match_confidence,
  match_reason,
  rule_version
from integration_relationship_evidence
where canonical_person_id in (<candidate person ids>)
   or emails::text ilike '%alicia.geigel%'
   or phones::text like '%7274201806%'
   or phones::text like '%8609895020%'
order by source, source_identity_key;
```

This query is critical because Apple Calls/FaceTime evidence may be `unmatched`, `review_required`, or linked to a different Person.

## 13.5 Inspect current Person x Source MV

```sql
select *
from mv_client_relationship_channels
where person_id in (<candidate person ids>)
order by person_id, source;
```

If evidence is exact-linked to the correct Person but missing here, then the diagnosis changes and the MV needs investigation. Until proven otherwise, the MV is believed correct.

## 13.6 Inspect canonical interactions

First inspect the exact interaction schema if necessary, then aggregate by Person/source:

```sql
select
  person_id,
  source_system,
  direction,
  count(*) as n,
  min(occurred_at) as first_at,
  max(occurred_at) as last_at
from interaction
where person_id in (<candidate person ids>)
group by person_id, source_system, direction
order by person_id, source_system, direction;
```

Expected new sources include:

```text
apple_calls
apple_facetime
```

## 13.7 Inspect current Apple Contacts projection

```sql
select
  lp.id,
  lp.source,
  lp.source_account,
  lp.source_contact_id,
  lp.display_name,
  lp.organization,
  li.identity_type,
  li.identity_value,
  li.normalized_value
from l_person lp
left join l_person_identity li on li.l_person_id = lp.id
where lower(coalesce(lp.display_name,'')) like '%alicia%'
   or lower(coalesce(lp.display_name,'')) like '%ami%'
   or lower(coalesce(li.identity_value,'')) like '%alicia.geigel%'
   or regexp_replace(coalesce(li.identity_value,''), '[^0-9]', '', 'g') like '%7274201806%'
   or regexp_replace(coalesce(li.identity_value,''), '[^0-9]', '', 'g') like '%8609895020%'
order by lp.source, lp.display_name, lp.id, li.ordinal, li.id;
```

This will reveal whether one Apple Contact truly contains all expected children or whether the UI-visible Alicia records originate from separate Apple/source profiles.

---

# 14. Questions the next engineer must answer before changing code

For Alicia:

1. How many active `person` rows actually correspond to her?
2. Which Person owns `alicia.geigel@gmail.com`?
3. Which Person owns `alicia.geigel@yahoo.com`?
4. Which Person owns phone `727-420-1806`?
5. Does the current Apple Contacts profile contain all of those identities in one source contact?
6. Which `integration_source_person_link` rows exist for Apple Contacts, Apple Messages, Gmail, Apple Calls, and FaceTime?
7. Which relationship evidence rows are `exact_linked`, `review_required`, `unmatched`, or `deferred`?
8. Are the 42 observed communications on the lowercase Alicia row Apple calls/messages, or something else?
9. Did the new call-history run materialize Alicia's call/FaceTime interactions onto a different Person?
10. Is any source handle already durably linked to a wrong Person due to an old loader?

For Ami:

1. Does Ami have a second phone in Apple Contacts or another source?
2. Are Apple call/FaceTime handles for that second phone exact-linked to Ami or stranded?
3. Are iMessage and call source handles linked to the same Person?

Do not merge anything until these questions are answered from Production data.

---

# 15. Likely defect classes once the graph is visible

## Case A: one source profile contains multiple identities, but one child is on another Person

If a current authoritative Apple Contact really contains Gmail + Yahoo + phone, yet source ownership is on Person A while one identity remains on Person B, this is a true legacy split.

Fix approach:

- repair source-linked Person consolidation
- move all child identities and all Person FK children transactionally
- update all source links/evidence that currently point to loser only when deterministic ownership is proven
- archive loser after all references are moved
- abort on uniqueness conflict, do not swallow
- refresh read models

The current `repair-source-person-splits.ts` may need to be broadened based on the actual evidence shape, but only after seeing why the current safe rule found zero candidates.

## Case B: the identities are actually in separate Apple/source profiles

Then it may not be a legacy split at all. The system needs deterministic cross-source mastering based on exact email/phone ownership.

Expected rule:

```text
explicit source link
  -> exact email
  -> exact semantic phone
  -> if all exact identities converge to one Person, attach/enrich
  -> if exact identities point at multiple Persons, review_required
```

Do not merge solely by display name.

## Case C: source link is already wrong

`integration_source_person_link` is deliberately match-once/enrich-forever. Normal mastering will not silently redirect an established source link.

If an old loader established an incorrect durable source owner, create an explicit bounded repair with auditability. Do not weaken the normal invariant globally.

## Case D: identities are correct, evidence is wrong

If all canonical/source ownership points to the same Person but `integration_relationship_evidence.canonical_person_id` is stale/different, repair the evidence reconciliation outcome and rerun materialization/read-model refresh.

## Case E: evidence is correct, interaction materialization is wrong

Apple Messages and Apple CallHistory materializers only write interactions for exact-linked source identities. If exact-linked evidence is right but interaction points elsewhere/missing, fix materialization/replay logic.

## Case F: all upstream data is right but MV/UI is wrong

Only then investigate `mv_client_relationship_channels`, `db/contact-history.ts`, and `components/portal/contact-history.tsx`.

Do not start here unless upstream is proven correct.

---

# 16. Apple Calls / FaceTime acceptance rules

The new call-history source should remain separate from Apple Messages ingestion.

Pipeline:

```text
Apple CallHistory DB (read only)
  -> JSONL export
  -> relationship evidence by remote address
  -> exact identity reconciliation
  -> source-person ownership
  -> canonical interaction
  -> Client relationship/contact history read models
```

Required fields currently available / expected:

- stable call UUID / row ID
- remote address (phone/email)
- start date/time
- direction
- duration
- call/service type
- answered/missed status if available
- local/account provenance

Do not infer FaceTime just because a Person has a phone number. It must come from actual CallHistory evidence.

The 2026-09-02 run proved 100 FaceTime records exist.

---

# 17. UI rule

Keep the current Contact History source positions including:

```text
PHONE
IMESSAGE
WHATSAPP
GMAIL
FACETIME
APPLE CALENDAR
```

The user explicitly wants the PHONE and FACETIME positions kept even while data integration is being completed.

Do not remove or collapse those slots as a workaround.

---

# 18. Tests that must exist before finalizing the identity fix

## Alicia-style fixture

One canonical Person with:

```text
phone A
phone B if present
Gmail email
Yahoo email if proven same source person
Apple Messages handle(s)
Gmail source identity
Apple Call handle
FaceTime handle
```

Assertions:

- exactly one active canonical Person
- all safe deterministic `person_identity` rows owned by same Person
- all durable source links owned by same Person
- evidence exact-linked to same Person
- one relationship row per Person x Source
- no duplicate Person from a new communication path
- detailed interactions remain source-specific
- replay does not duplicate Person, evidence, or interaction

## Ami-style fixture

One Person with two phone/Apple handles.

Assertions:

- both handles resolve to same Person
- iMessage aggregates both handles
- calls aggregate to same Person
- FaceTime aggregates to same Person when present

## Conflict fixture

Two exact identities genuinely point to two different established Persons.

Assertion:

- no auto-merge
- review_required / ambiguous
- no partial child migration

---

# 19. Safety / operating rules

1. Production is the dataset under investigation.
2. Read-only discovery first.
3. Do not guess Person ownership from names alone.
4. Do not weaken the existing fail-closed mastering rules globally.
5. Do not delete communication evidence to make duplicates disappear.
6. Do not mutate already-issued/signed form/document data while working on relationship identity.
7. Person consolidation must be transactional.
8. Any uniqueness conflict during Person consolidation must abort, not be ignored.
9. Refresh the Client read models after successful canonical ownership changes.
10. Prefer targeted tests; do not run the full ~2-hour regression unless explicitly authorized.

---

# 20. Suggested next sequence for the new chat / engineer

1. Restore/reconnect Neon plugin if necessary.
2. Run only `select 1` first and confirm Production branch/database.
3. Execute the Alicia/Ami graph queries from Section 13.
4. Write down exact Person IDs and identity/source/evidence ownership.
5. Classify the defect using Section 15.
6. Patch the smallest upstream seam that explains the real rows.
7. Add Alicia + Ami targeted regression tests.
8. Apply bounded Production repair only after the deterministic rule is proven.
9. Refresh read models.
10. Reload Alicia Client Contact History and verify Gmail + iMessage + Phone + FaceTime converge onto one Person.
11. Then check Ami as the second fixture.
12. Only after identity convergence is correct, continue broader FaceTime / call-history polishing.

---

# 21. Current state in one paragraph

The data model is already intended to be **one Person with one-to-many communication identities and Person x Source relationship aggregation**. Semantic NANP normalization has been fixed in reconciliation/mastering. Apple call-history ingestion has been added and successfully imported 4,980 calls, including 100 FaceTime calls, with 4,510 canonical interactions inserted and 470 calls skipped because the source identity was not linked. Alicia still appears fragmented in the Client UI, but the new conservative Apple-source Person consolidation script found zero eligible legacy splits, so the exact Production identity/source/evidence graph must be inspected before changing merge rules. The Neon connector wrapper is currently the blocker to that read-only inspection. The next engineer should restore Neon access, trace Alicia and Ami end-to-end, then fix the precise upstream ownership/reconciliation seam rather than changing the Person-first architecture or the relationship MV blindly.

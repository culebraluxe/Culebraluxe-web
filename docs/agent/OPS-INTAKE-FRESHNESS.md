# Intake freshness — what the rows say

Owner: DEV_OPS. Measured **2026-10-05** against **PROD**, read-only, with the repository's own read tool
(`forge sql --target prod`, which refuses anything but a SELECT and refuses to run without naming its database).

Re-run any line of it — the answer comes from the rows, never a log:

```sh
export CARGO_TARGET_DIR=/Users/Shared/dev/build/rust
cargo run --quiet -p cli -- forge sql --target prod --sql "<one of the two queries below>"
```

```sql
-- Q1: the shared ODS seam, per source and account — landed, processed, newest
select source, source_account, count(*)::text n,
       count(*) filter (where processing_completed_at is not null)::text processed,
       max(observed_at)::text newest
from integration_inbox group by source, source_account order by max(observed_at) desc

-- Q2: the ODS landing tables, one row, every chain at once
select (select max(ingested_at)::text from l_imessage) imessage_newest,
       (select max(ingested_at)::text from l_call)     calls_newest,
       (select max(ingested_at)::text from l_applemail) mail_newest,
       (select max(ingested_at)::text from l_whatsapp)  whatsapp_newest,
       (select max(projected_at)::text from l_person)   contacts_projected_newest
```

## What the rows said on 2026-10-05

| chain | ODS landing | volume | newest landing | verdict |
| --- | --- | --- | --- | --- |
| WhatsApp intake — `meta-1330394873483351` | `integration_inbox` → `l_whatsapp` | 1156 seam rows **1156 processed (100%)**, 1160 landed | **2026-10-05 08:31** | **alive**, daily 2026-09-24 → 2026-10-05 (34–202/day) |
| Apple Messages / iMessage (local listener) | `l_imessage` | 48,970 | **2026-10-04 22:14** | **alive** |
| Apple Contacts | `integration_intake_batch` → `l_person` | 18 batches, all `loaded` | **2026-09-22 19:33** (13 days) | idle, not exercised since 2026-10-03 refactor |
| Apple Calls | `l_call` | 5,738 | 2026-09-28 20:27 | idle |
| Apple Mail / Gmail | `l_applemail` | 3,839 | 2026-09-28 20:33 | idle |
| WhatsApp — `meta-1269382202929480` | `integration_inbox` | 8 landed, 5 processed | 2026-09-15 | retired account; 3 rows never processed |

The WhatsApp answer the Captain asked for, in one line: **the intake landed and fully processed rows on 2026-10-02
(23), 2026-10-03 (28 — the day the tree was rebuilt), 2026-10-04 (36) and 2026-10-05 (4 before 08:31)**, so neither the
layout refactor (`80cfc9da`, `da16f5ea`, 2026-10-03) nor the Phase 2 capture change (`40fc3285`, 2026-10-04) broke it.

## How to read this file without fooling yourself

1. **A stale landing table usually means the source did not run, not that it is broken.** Check the local export's
   mtime before calling it a defect. On 2026-10-05 every disk/DB pair agreed — `contact-export/contacts-export.json`
   Sep 22 15:33 ↔ last batch Sep 22; `.../apple-messages-export/calls.jsonl` Sep 28 16:27 ↔ `l_call` Sep 28 — so the
   chains were idle, and nothing showed as broken.
2. **Contacts inbox rows are receipts, not events: they are never `processing_completed_at`.** 0 processed for
   `apple_contacts` is by design (migration 254 does not touch the column); the processing of a contacts batch is
   `apple_contacts_project` → `l_person`. Do not read it as a stuck backlog.
3. **Idle is not verified.** `apple_contacts_load(p_payload jsonb, p_source_account text)` and
   `apple_contacts_project(p_source_account text, p_batch_id uuid)` both exist in PROD, so an idle chain would find its
   code — but a chain that has not run since the refactor has produced no evidence about the refactor. One run is the
   proof, and the export step needs the Mac's TCC grant (`pnpm apple:sync:verify-tcc`).
4. **`forge sql` cannot read a query that aliases a column `t`.** The row builder wraps your statement as
   `select row_to_json(t)::text as row from (<sql>) t limit $1` (`db/src/forge_read.rs:626` on `main`; line 659 in a
   working tree carrying another lane's in-flight edits), so the alias shadows the
   wrapper's table alias and the tool dies with `function row_to_json(text) does not exist`. Alias it something else
   (or fix the wrapper to use a namespaced alias) — a legitimate SELECT that fails looks like a database problem and is
   not one.

## Open — owner: the Captain

1. Apple Contacts, Calls and Mail have not run since the 2026-10-03 refactor (last runs 09-22, 09-28, 09-28). One run
   of each is the only thing that answers "did the refactor break them?".
2. The retired Meta account `meta-1269382202929480` still holds 3 unprocessed rows (≤2026-09-15): retire the rows, or
   is the number expected back?

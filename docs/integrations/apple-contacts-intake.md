# Apple Contacts Intake — Architecture & Runbook

**SUPPORT-2** · Relational load projection + Clients visibility
**Status:** current · **Docs date:** 2026-08-24

This document is the practical guide for the Apple Contacts intake pipeline. It
exists so a future engineer never builds a shadow ingestion process: the ODS
staging layer, the relational load projection, and canonical promotion all follow
one explicit spine.

---

## 1. Complete path

```text
Apple Contacts (Swift exporter)
  -> contacts-export.json   (PRIVATE, gitignored)
  -> generic intake layer   (integration_inbox)
  -> staged profile         (integration_staged_contact_profile, immutable revisions)
  -> relational load        (l_person + l_person_identity + l_person_address)   [SUPPORT-2]
  -> reconciliation         (future)  ->  canonical person / person_identity  ->  Clients
```

- The **exporter** (`contact-export/`, Swift) dumps a CNContact export to JSON.
- The **generic intake layer** (`lib/intake/*`, `db/integration-inbox.ts`) owns
  source payload, batch accounting, immutable revisions, fingerprints, and replay
  history.
- The **relational load** (`l_person*`, `scripts/project-apple-contacts.ts`) is a
  current-state projection of the latest staged revision — visible and usable,
  but NOT canonical.
- **Canonical promotion** (merging into `person` / `person_identity` / Clients) is
  intentionally NOT built yet.

---

## 2. Table ownership

| Table | Owner | Purpose | Canonical? |
|---|---|---|---|
| `integration_intake_batch` | generic intake | one row per exported batch (provenance, balance) | No (staging) |
| `integration_inbox` | generic intake | durable source receipt / identity | No (staging) |
| `integration_staged_contact_profile` | generic intake | one **immutable revision** per contact (JSONB profile) | No (staging) |
| `l_person` | relational-load projection | one current-state row per (source, source_account, source_contact_id) | No (load) |
| `l_person_identity` | relational-load projection | labeled email/phone/apple_contact children | No (load) |
| `l_person_address` | relational-load projection | postal-address children (addresses are not identities) | No (load) |
| `person` | canonical CRM | canonical Client / Person | **Yes** |
| `person_identity` | canonical CRM | canonical identities | **Yes** |
| `deal`, `interaction`, `task`, `deal_participant`, … | canonical CRM | canonical business records | **Yes** |

No `l_client` table exists. Canonical Clients are `person` + `person_identity`.

---

## 3. Exporting another Apple Contacts batch

1. Build/run the Swift exporter under `contact-export/` to produce
   `contact-export/contacts-export.json`.
2. The loader (`scripts/load-apple-contacts.ts`) validates the export, lowers each
   contact through the canonical intake lane, and writes immutable staged
   revisions.

---

## 4. Where the private JSON belongs (and why it is ignored)

- The private export lives at `contact-export/contacts-export.json`.
- It is listed in `.gitignore` and is **never committed**.
- All pipeline code reads it only at load time and never logs individual contact
  payloads. Staged `profile` JSONB lives in Neon (authoritative), not in Git.
- Guardrail: never `git add .`; stage exact paths only. Never commit
  `contacts-export.json` or any private contact data.

---

## 5. Load commands (DEV / Production)

```sh
# DEV
pnpm contacts:load:dev --file contact-export/contacts-export.json --source-account <account>

# Production
pnpm contacts:load:prod --file contact-export/contacts-export.json --source-account <account>
```

`--env dev|prod` selects `DATABASE_URL_DEV` / `DATABASE_URL_PROD`. The loader fails
closed on empty `--source-account` and on DEV/PROD URL ambiguity.

---

## 6. Projection command

```sh
pnpm contacts:project:dev     # DATABASE_URL_DEV
pnpm contacts:project:prod    # DATABASE_URL_PROD
```

`scripts/project-apple-contacts.ts` reads the **latest staged revision** per
identity, upserts one `l_person` current-state row, and deterministically
replaces its `l_person_identity` + `l_person_address` children. It never mutates
canonical `person` / `person_identity`, and it does not require re-exporting.

---

## 7. Verification queries / counts


---

## 8. Identical replay behavior

Re-running the projection with the same staged revisions is a no-op:

- `l_person` is upserted by `unique(source, source_account, source_contact_id)`
  (no duplicate load people).
- `l_person_identity` is constrained by
  `unique(l_person_id, identity_type, identity_value)` (no duplicate identities).
- Child rows are deterministically rebuilt, so replay produces **zero additional**
  load people or identities.

---

## 9. Changed-contact revision behavior

A changed contact (new payload fingerprint) produces a **new immutable staged
revision** (linked via `supersedes_profile_id`). The projection picks the latest
revision per identity and **updates the existing `l_person` row** (name/org/
address) and rebuilds its children. The older staged revision is preserved
immutably; the load projection always reflects the current one.

---

## 10. Failure inspection and recovery

- Loader: check `integration_intake_batch.load_status`; a `conflict` means the
  same batch id arrived with a different checksum (safe replay only with the
  identical file).
- Projection: each contact runs in its own transaction; a failed contact rolls
  back and is reported in `firstErrors`, the rest continue. Re-run to retry.
- To inspect a specific contact's staged revision and its load row:

```sql
select * from integration_staged_contact_profile where source_contact_id = '…';
select * from l_person where source_contact_id = '…';
select * from l_person_identity where l_person_id = (select id from l_person where source_contact_id = '…');
```

---

## 11. Canonical tables — do not reset casually

`person`, `person_identity`, `deal`, `interaction`, `task`, `deal_participant`,
`offer`, and every other canonical CRM table are system-of-record. Never
truncate/reset them casually. Staging and load tables
(`integration_*`, `l_*`) are rebuildable projections; canonical tables are not.

---

## 12. Current boundary

Relational load is **complete**; canonical promotion is **later**. Imported
contacts are visible in Clients under **Imported Contacts**, clearly labelled
Apple Contacts / Imported / Unreviewed — they are **not** canonical CRM Clients
and have no promote/merge/reject workflow yet.

---

## 13. Future reuse of the neutral intake spine

Email, Calendar, iMessage metadata, call metadata, and WhatsApp adapters can ride
the same generic intake spine (`integration_inbox` →
`integration_staged_*` → relational load → reconciliation → canonical). Each
adapter contributes source facts through `lib/intake/*`; the ODS staging,
fingerprinting, immutable revisions, replay history, and (later) reconciliation
are shared. Do not build a parallel ingestion pipeline per channel.

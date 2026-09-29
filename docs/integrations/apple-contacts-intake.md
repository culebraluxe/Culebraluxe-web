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
- The **generic intake layer** (`lib/intake/*`, `legacy/db/integration-inbox.ts`) owns
  source payload, batch accounting, immutable revisions, fingerprints, and replay
  history. *(Historical: that TypeScript is deleted; the rules live in
  `db/migrations/254_apple_contacts_load_project.sql` now.)*
- The **relational load** (`l_person*`, **`apple-sync contacts-project`** /
  `apple_contacts_project` — was `scripts/project-apple-contacts.ts`) is a
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
2. The loader (**`apple-sync contacts-load`** / `apple_contacts_load`) validates the
   export, lowers each contact through the canonical intake lane, and writes
   immutable staged revisions — set-based, inside Neon.

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
cargo run --manifest-path rust/Cargo.toml -p cli -- apple-sync contacts-project dev
cargo run --manifest-path rust/Cargo.toml -p cli -- apple-sync contacts-project prod
```

`apple_contacts_project` (migration 254) reads the **latest staged revision** per
identity, upserts one `l_person` current-state row, and replaces its phones/emails
plus the `l_property` addresses in one transaction. It never mutates
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

## 9.1 The canonical profile text is the revision identity — trim it like JavaScript

`apple_contacts_fingerprint` is `sha256(apple_contacts_profile_text(contact))`, so the
text is not a display detail: it **is** what decides new / replay / changed. The text
is `JSON.stringify(normalizeProfile(contact))` of the retired loader
(`scripts/load-apple-contacts.ts:119-153`), including its `.trim()` on every field.

**`btrim(x)` is not `.trim()`.** Postgres `btrim` with one argument strips **spaces
only**; JavaScript's `.trim()` strips TAB, LF, VT, FF, CR, SP, NBSP, U+2000–U+200A,
U+2028/9, U+202F, U+205F, U+3000 and ZWNBSP. Migration 254 used `btrim`, so a street of
`E'\nBo. Delicias 17a'` was kept where the retired loader trimmed it — found by the
PROD load on 2026-09-28 (2854 replay / **1 changed**, differing only in a leading
newline on a postal street). `db/migrations/255_apple_trim.sql` adds `apple_trim(text)`
(the JavaScript character set, tested against `node`'s `String.prototype.trim`) and
rebuilds the three canonical-text functions on it. Re-project and re-load after any
change to those functions; `apple_contacts_fingerprint_audit()` is the check.

**One consequence worth knowing before reading the audit.** The staged insert dedupes
on `(source, source_account, source_contact_id, payload_fingerprint)` — the retired
loader's own rule — so when a corrected normalizer returns a text the ODS already holds
as an *older* revision, the load writes nothing (it counts as replay) and the newer
revision stays the latest one, which is what the projection reads by `revision desc`.
A wrong-but-stored revision is therefore not self-healing: it has to be removed (a
destructive change, so a Captain decision), and the audit reports it as
`mismatched: 1` until then.

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

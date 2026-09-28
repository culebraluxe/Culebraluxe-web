> **UPDATE 2026-09-28 (afternoon) — step 3 of §5 is DONE, and the architecture changed.**
> The captain refused the shape of the port, not just its absence: *"you have to pull all the data out to
> RUST mutate then push back to DB — not the correct architecture"*. The landing → warehouse hop is a
> set-based transformation, so it lives in Neon as a function and Rust is only the caller:
>
>   * `db/migrations/253_apple_contacts_promote.sql` — `normalize_identity_email`, `normalize_identity_phone`
>     (the Rust domain rules restated in SQL, because the matching happens in SQL) and
>     `warehouse_promote_apple_contacts(p_apply boolean)` — plan, tallies and every write, set-based.
>   * `rust/cli/src/apple_contacts.rs` `warehouse_promote` + `apple-sync warehouse-promote [dev|prod] [--apply]`,
>     thin caller, `--apply` refreshes the client read models afterwards (a materialized view cannot be
>     refreshed `concurrently` inside a function's transaction).
>   * `scripts/contacts-sync.sh:158` repointed; `promote-warehouse.ts` is dead and stays dead.
>
> Applied and run: DEV dry → apply → apply again (identical tallies, counts stable), then PROD
> (2855 landing rows, 2814 matched, 41 skipped no identity, 0 ambiguous, 11 places created, 11 linked,
> 0 legacy links retired, read models refreshed). **Steps 1 (load) and 2 (project) are still TS and still
> dead — and they are to be ported the same way: SQL functions with a thin caller, not scripts.**


# Handoff — the Contacts chain (`contacts-sync.sh`), 2026-09-28

The Apple/contacts/mail/Gmail chain was broken by the Rust port: four shell wrappers called deleted
TypeScript. Mail, Gmail, Messages and Calls are done. **Contacts is the one left**, and
`scripts/contacts-sync.sh` still cannot run past its first dead line.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The scheduled Apple Messages job runs the Rust intake again; its last failure was the deleted script and nothing else | `pnpm apple:sync:status` — wrapper is `scripts/apple-sync.sh`, `cargo run … apple-sync messages-intake`, last failure 2026-09-28T12:04 was `apple-messages-intake.ts` |
| S2 | Full Disk Access survived the launcher rebuild | `pnpm apple:sync:verify-tcc` → `TCC VERIFY: OK chat.db opened READ-ONLY message_count=95084` |
| S3 | `apple-calls-sync.sh` is repointed and the whole calls chain is Rust | `git show c292beea --stat`, wrapper line ~41 |
| S4 | The contacts notes step is Rust | `rust/cli/src/apple_contacts.rs`; `cargo run -p cli -- apple-sync contacts-notes --file <export.json>` |
| S5 | **`contacts-sync.sh:130,135,143,149` still call deleted TypeScript** — ported so far: line 130 only | `grep -n 'import tsx' scripts/contacts-sync.sh` |
| S6 | Contacts is the chain that writes `person`/`property` (and `db/migrations/228_person_merge.sql` reads it) — this is the data-integrity-sensitive path | `scripts/promote-warehouse.ts` header; inventory §"Live callers" |
| S7 | The mail chain touches no canonical row: `person` 2683→2683, `property` 2156→2156 on the DEV promotion | `docs/agent/HANDOFF-forge-port-2026-09-28.md` §9 |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | `run prod email`, `run prod calls`, any PROD contacts run | the Captain | ask before the first PROD write; DEV is free |
| H2 | Re-granting Full Disk Access | the Captain (one click) | if TCC verify fails, ask; never grant it yourself |
| H3 | The failing `catch_up` test | whoever landed `4ef9b58d` | do not "fix" it inside a contacts story — it fails on the clean tree too |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Port the ODS load | `scripts/load-apple-contacts.ts` (reference only) + its deleted deps: `git show 4cf98110^:lib/intake/apple-contacts.ts`, `…:lib/intake/batch.ts`, `…:legacy/db/integration-inbox.ts` | `rust/cli/src/apple_contacts.rs`, `rust/core/db/src/{apple_ods,intake}.rs` |
| Port the `l_person`/`l_property` projection | `scripts/project-apple-contacts.ts` — the SQL is in §5 | same |
| Port the warehouse promotion (DEV_OPS P0) | `scripts/promote-warehouse.ts` — SQL and rules in §5 | `rust/cli/src/apple_contacts.rs`, `db::PersonDao` if a statement is missing |
| Repoint the wrapper | `scripts/contacts-sync.sh:130,135,143,149` | that file, then `docs/agent/BROKEN-TS-INVENTORY.md:64` |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `c292beea` | Calls chain ported; `apple-calls-sync.sh` repointed; inventory updated | `cargo check --workspace --all-targets` (exit 0), `cargo test -p domain apple_calls` (6 passed), live DEV run |
| `82030fed` | Contacts notes merge ported (`apple-sync contacts-notes`) | `cargo check -p cli --all-targets`, `cargo test -p cli apple_contacts` (2 passed), live run on a copy of the export: 21 people with notes, 9/2855 merged, 2.1s |

## 5. The extracted design — port this, do not re-read the TypeScript

Three commands remain, and their order matters: `load` → `project` → `promote`. Each is PROD-only in
the wrapper and must fail closed on a missing or DEV-identical `DATABASE_URL_PROD`.

**`contacts-load --env prod --file <export.json> --source-account <a>`** (from `load-apple-contacts.ts`)

1. `sha256` the raw file; parse the batch (`schemaVersion`, `sourceSystem: apple_contacts`,
   `exportId`, `exportedAt`, `contacts[]`); every field a required string except `note`.
2. Batch receipt: `insert into integration_intake_batch (source, source_account, external_batch_id,
   schema_version, exported_at, received_at, file_sha256, load_status, input_count, valid_count,
   new_profile_count, replay_count, changed_revision_count, error_count) values (…, 'processing',
   0,0,0,0,0,0) on conflict (source, source_account, external_batch_id) do nothing returning id`.
   On conflict: identical `file_sha256` → safe replay; different → `load_status='conflict'`, exit 1.
3. Per contact, in memory first: `normalizeProfile` (trim everything; emails/phones as
   `{label,value}` filtered on non-empty value and sorted by `label|value`; postal addresses sorted by
   `label|street|city`), then `sha256(JSON(profile))` is the payload fingerprint.
4. Bulk-read in chunks of 400: the latest staged revision per `source_contact_id`
   (`distinct on … order by revision desc`) and the existing inbox receipts
   (`integration_inbox.external_event_id`).
5. Classify: prior exists + same fingerprint + inbox receipt → **exact replay, no DB work at all**.
   Otherwise write: the inbox row (identity is `source + source_account + external_event_id =
   sourceId`), then `integration_staged_contact_profile` (`revision = prior+1 or 1`,
   `supersedes_profile_id`, `on conflict (source, source_account, source_contact_id,
   payload_fingerprint) do nothing` — a concurrent identical replay is a replay, not a change).
6. Snapshot membership for **every contact in this export, replays included**:
   `insert into integration_source_snapshot_member (integration_intake_batch_id, source,
   source_account, source_identity_key) select …, unnest($ids) on conflict do nothing`.
7. Update the batch counters and `load_status` (`failed` when `error_count > 0`), then exit non-zero
   after that durable accounting. Print only the non-PII JSON summary.


**`contacts-project --env prod`** (from `project-apple-contacts.ts`) — three statements in one
transaction, all keyed on the newest **loaded** batch:

- resolve `source_account` (exactly one, else refuse) and that batch;
- `LATEST_CTE` = current-snapshot members joined to their latest staged revision;
- upsert `l_person`: display name is `nullif(trim(concat_ws(' ', parts)), '')` with **each part
  nullif'd before the concat** (empty strings otherwise double the spaces), falling back to
  organization → nickname → `source_contact_id` → `(unnamed)`; phones/emails as
  `jsonb_agg({label,value})` filtered on non-empty value; `display_address` is the `_$!<Home>!$_`
  address or the first, built with `concat_ws(', ', street, city, state + postal, country)`;
- prune `l_person` rows whose `source_contact_id` is not a current-snapshot member;
- upsert `l_property` per postal address with `source_key = source_contact_id || ':' || (ordinal-1)`,
  address type by label (`%home%` → LEGAL, `%work%` → PHYSICAL, otherwise ordinal 0 → LEGAL / 1 →
  PHYSICAL / else OTHER), skipping rows with neither street nor city.

**`warehouse-promote --env prod [--apply]`** (from `promote-warehouse.ts`; dry run by default)

- `LANDING_SQL`: `l_person` for `source='apple_contacts'` plus its LEGAL address from `l_property`;
- identity is the normalized email/phone (`domain::apple_messages::{normalize_email,
  normalize_phone}`); read every `person_identity` once; exactly one owner → match, none → create,
  more than one → **ambiguous and left alone, never guessed**;
- create the new people (`display_name` = landing name or `(unnamed)`, role `unclassified`,
  identities primary-first, `source_system = 'apple_contacts'`);
- one `update person … from unnest(…)` where a non-empty landing value wins (and
  `display_name_source = 'apple_contacts'` when the name came from it), then one
  `insert into person_identity … where not exists`;
- `l_property` → `property` (`source_type`/`source_provider = 'apple_contacts'`,
  `source_listing_key = 'apple_contacts:' || source_key`, status `prospect`, no invented name,
  `iso_country_code = 'us'`) plus `person_property` (`LEGAL → legal_address`,
  `PHYSICAL → physical_property`, `on conflict do nothing`), then delete the legacy
  `relation_type = 'address'` links for those keys;
- with `--apply`, refresh the client read models.

## 6. NOT VERIFIED — the honest gaps

- Nothing in the Contacts chain has run in Rust: `load`, `project` and `promote` do not exist yet.
- `pnpm ui:check` was not run — neither commit touches `rust/ui`.
- `cargo test -p server catch_up::tests::snooze_is_bounded_and_handle_is_a_repository_command` fails
  on the clean tree as well (stashed, re-run, failed identically). Pre-existing, not from this work.
- The DEV calls run proves replay safety and the interaction write; it says nothing about PROD counts.

## 7. OPEN — the next actions, in order

1. Port `contacts-load` (§5). Validate on DEV: a replay of the same export writes no new staged
   revision and leaves snapshot membership unchanged.
2. Port `contacts-project` (§5). Validate on DEV: a second run leaves `l_person`/`l_property` counts
   and its own before/after/pruned totals identical.
3. Port `warehouse-promote` (§5). Validate on DEV: dry run, then `--apply`, leaves
   `person`/`property`/`person_identity` counts stable and creates the legal/physical links.
4. Repoint `scripts/contacts-sync.sh` lines 130/135/143/149 to the four Rust commands, and update
   `docs/agent/BROKEN-TS-INVENTORY.md:64`. Finished when the whole wrapper runs export → ODS →
   `l_person`/`l_property` → warehouse on DEV.
5. Only then ask the Captain for the PROD run (H1).

## 8. ASK THE OWNER

- "Run the contacts chain against PROD?" — yes: run `bash scripts/contacts-sync.sh` and report the
  four JSON tallies; no: leave it DEV-verified and say so.
- "Re-arm the scheduled job again?" — only if `pnpm apple:sync:verify-tcc` fails after a macOS
  update: `pnpm apple:sync:install`, then re-grant Full Disk Access when macOS asks.
- "Gmail API fallback?" — `rust/cli gmail-sync` needs `GOOGLE_CLIENT_ID`/`_SECRET`/`_REFRESH_TOKEN`
  only if the local Mail.app `[Gmail]/All Mail` route ever misses mail; today it does not.


# Bank OFX / QBO Intake — operator note

CulebraLuxe can accept a QuickBooks Web Connect **`.qbo`** file or an ordinary
**`.ofx`** bank file and load its transactions **replay-safely** into the neutral
load table `public.l_bank_transaction`.

> **Scope:** this is **BANK EVIDENCE INTAKE ONLY.** No expenses are created, no
> vendors/categories are inferred, and no reconciliation to `account_expense`
> happens here. Future reconciliation is deliberately a separate step.

## 1. Download the bank file

From your bank (e.g. Banco Popular), export the statement as a
QuickBooks Web Connect (`.qbo`) or OFX (`.ofx`) file and save it locally.

## 2. Run a dry run first (inspect before persisting)

```sh
pnpm bank:dry-run -- /path/to/statement.qbo
```

This parses the file and prints a summary **without touching the database**:

```
File: statement.qbo
Format: QBO / legacy OFX
Account: ****6789
Bank ID: 999999999
Currency: USD
Period: 2026-07-01 → 2026-07-31

Parsed:    27
Valid:     27
Inserted:  0
Replay:    0
Rejected:  0

Debits:   -6,444.90
Credits:  32,402.34
```

Account numbers are masked; raw bank files are never printed.

## 3. Inspect the summary

- `Parsed` = total `<STMTTRN>` blocks found.
- `Valid` = transactions that will be loaded.
- `Rejected` = transactions quarantined (missing FITID, invalid amount, or
  unusable posted date) — they are counted, not fabricated.
- `Debits` / `Credits` = signed totals of the valid transactions.

## 4. Run the load

```sh
pnpm bank:load:dev -- /path/to/statement.qbo   # DEV
pnpm bank:load:prod -- /path/to/statement.qbo  # PROD
```

The loader inserts normalized transactions replay-safely (unique
`(source_system, source_account, fitid)`), then prints the same summary with the
actual `Inserted` / `Replay` / `Rejected` counts.

## 5. Rerunning the same file is safe

The unique constraint is the final backstop:

- **First load:** `Inserted: N`.
- **Same file again:** `Inserted: 0, Replay: N` — no duplicate rows, no failure.
- **Overlapping file:** existing FITIDs are replayed (skipped), new FITIDs are
  inserted.
- Replay never overwrites an existing row's reconciliation state.

---

### Supported formats

- Legacy OFX 1.x / SGML-style QBO (e.g. `OFXHEADER:` / `DATA:OFXSGML`).
- OFX 2.x XML (begins with an XML declaration or `<OFX>`).
- Dates: `YYYYMMDD`, `YYYYMMDDHHMMSS`, fractional seconds, and OFX timezone
  suffixes (`[-4:AST]`, `[0:GMT]`) — normalized consistently to ISO UTC.

Files that cannot be identified as OFX-family are rejected.

### Out of scope (not built here)

Automatic expense creation, vendor/category inference, receipt OCR, QuickBooks /
bank APIs, OAuth, Plaid, reconciliation UI, automatic posting, scheduled
downloads, or CSV/other file formats. This is one bounded OFX/QBO intake seam.

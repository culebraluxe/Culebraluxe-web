-- ===========================================================================
-- ACCOUNTING V1 — OFX/QBO bank transaction load projection
--
-- Neutral load boundary for bank statement transactions imported from QBO or
-- OFX files.  The accounting application tables remain canonical; this table
-- preserves source transaction identity and bank facts for replay-safe import,
-- enrichment, matching, and reconciliation.
--
-- QBO is an Intuit/Web Connect flavor of OFX.  Legacy QBO/OFX payloads may be
-- SGML rather than XML, so raw source fragments are intentionally stored as
-- text rather than PostgreSQL xml.
-- ===========================================================================

create table if not exists public.l_bank_transaction (
    id uuid primary key default gen_random_uuid(),

    source_system text not null default 'ofx_qbo',
    source_format text not null,
    source_account text not null,
    bank_id text,
    account_type text,
    currency_code text,

    fitid text not null,
    transaction_type text,
    posted_at timestamptz not null,
    user_initiated_at timestamptz,
    amount numeric(14,2) not null,
    payee_name text,
    memo text,
    check_number text,
    reference_number text,

    source_file_sha256 text,
    raw_source_fragment text,

    reconciliation_status text not null default 'unmatched',
    matched_account_expense_id uuid references public.account_expense(id),
    reconciliation_note text,

    imported_at timestamptz not null default now(),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint l_bank_transaction_source_format_ck
      check (source_format in ('QBO', 'OFX')),
    constraint l_bank_transaction_reconciliation_status_ck
      check (reconciliation_status in ('unmatched', 'matched', 'ignored', 'review')),
    constraint l_bank_transaction_source_identity_uq
      unique (source_system, source_account, fitid)
);

create index if not exists l_bank_transaction_posted_at_idx
    on public.l_bank_transaction (posted_at desc);

create index if not exists l_bank_transaction_reconciliation_idx
    on public.l_bank_transaction (reconciliation_status, posted_at desc);

create index if not exists l_bank_transaction_expense_idx
    on public.l_bank_transaction (matched_account_expense_id)
    where matched_account_expense_id is not null;

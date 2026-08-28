-- ===========================================================================
-- ACCOUNTING V1 — indexes that already exist in DEV and were created after
-- migration 087 (which only created the tables). Brings PROD up to the same
-- operational schema level DEV is at. Idempotent so it is a safe no-op where
-- the indexes already exist.
--
-- These are the index shapes DEV already uses for the accounting dashboard
-- queries (status/due filters on account_receivable; date+category and deal
-- filters on account_expense).
-- ===========================================================================

create index if not exists account_receivable_status_due_idx
    on public.account_receivable (status, due_on);

create index if not exists account_receivable_deal_idx
    on public.account_receivable (deal_id) where (deal_id is not null);

create index if not exists account_expense_date_category_idx
    on public.account_expense (expense_on, category);

create index if not exists account_expense_deal_idx
    on public.account_expense (deal_id) where (deal_id is not null);

begin;

alter table offer
    add column if not exists financing_type text,
    add column if not exists deposit_amount numeric(14,2),
    add column if not exists inspection_days integer,
    add column if not exists seller_credits numeric(14,2),
    add column if not exists proposed_closing_date date,
    add column if not exists contingencies text,
    add column if not exists expires_at timestamptz;

do $$
begin
    if not exists (
        select 1 from pg_constraint where conname = 'offer_inspection_days_range'
    ) then
        alter table offer
            add constraint offer_inspection_days_range
            check (inspection_days is null or inspection_days between 0 and 365);
    end if;
    if not exists (
        select 1 from pg_constraint where conname = 'offer_deposit_nonnegative'
    ) then
        alter table offer
            add constraint offer_deposit_nonnegative
            check (deposit_amount is null or deposit_amount >= 0);
    end if;
    if not exists (
        select 1 from pg_constraint where conname = 'offer_seller_credits_nonnegative'
    ) then
        alter table offer
            add constraint offer_seller_credits_nonnegative
            check (seller_credits is null or seller_credits >= 0);
    end if;
end $$;

commit;

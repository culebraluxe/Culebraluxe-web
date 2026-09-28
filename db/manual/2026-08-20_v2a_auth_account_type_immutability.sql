-- CulebraLuxe Portal
-- AUTH-01 corrective patch — account_type immutability enforcement
-- Date: 2026-08-20
-- Source: review-approved fix for migration 015
--
-- Adds the account_type immutability guard that closes the reverse-mutation
-- hole for internal/external role assignments. Additive and idempotent; safe
-- to re-run. It does NOT touch the existing enforce_app_user_role_account_type
-- trigger (already live).
--
-- Run this manually against the deployed database with human authorization.
-- Do NOT rerun migration 015.

begin;

create or replace function prevent_account_type_change()
returns trigger
language plpgsql
as $$
begin
    if new.account_type is distinct from old.account_type then
        raise exception 'account_type is immutable once referenced';
    end if;
    return new;
end;
$$;

drop trigger if exists prevent_app_user_account_type_change on app_user;

create trigger prevent_app_user_account_type_change
    before update of account_type on app_user
    for each row
    execute function prevent_account_type_change();

drop trigger if exists prevent_role_account_type_change on role;

create trigger prevent_role_account_type_change
    before update of account_type on role
    for each row
    execute function prevent_account_type_change();

commit;

-- ==================================================
-- POSTFLIGHT — read-only
-- ==================================================

-- Expect exactly these two triggers present.
select tgname
from pg_trigger
where tgname in (
    'prevent_app_user_account_type_change',
    'prevent_role_account_type_change'
)
order by tgname;

-- CulebraLuxe
-- Reclaim `role` for business/domain positions; make authorization naming explicit.
-- Migration: 118_security_role_business_role.sql
--
-- Final vocabulary after this migration:
--   security_role  = application authorization bundle (ROOT, BUSINESS_POWER, ...)
--   role           = scoped business/domain position (BUYER, SELLER, LENDER, ...)
--   role_alias     = accepted boundary vocabulary for a canonical business Role
--
-- Existing FK relationships follow PostgreSQL table renames automatically.
-- No authorization assignments or business-role rows are rewritten.

begin;

-- AUTH historically claimed the short table name `role`. Give it the name it
-- always meant before reclaiming `role` for the business vocabulary.
do $$
begin
    if to_regclass('public.security_role') is null
       and to_regclass('public.role') is not null
       and exists (
           select 1
           from information_schema.columns
           where table_schema = 'public'
             and table_name = 'role'
             and column_name = 'account_type'
       ) then
        alter table role rename to security_role;
    end if;
end
$$;

-- PNS-LENS-01 used the temporary collision-avoidance name relation_role.
-- Reclaim the short canonical business name now that AUTH is explicit.
do $$
begin
    if to_regclass('public.role') is null
       and to_regclass('public.relation_role') is not null then
        alter table relation_role rename to role;
    end if;
end
$$;

do $$
begin
    if to_regclass('public.role_alias') is null
       and to_regclass('public.relation_role_alias') is not null then
        alter table relation_role_alias rename to role_alias;
    end if;
end
$$;

-- Stored PL/pgSQL bodies resolve relation names when executed, so refresh the
-- one security function that previously referenced `role` by name.
create or replace function enforce_app_user_role_account_type()
returns trigger
language plpgsql
as $$
declare
    v_user_account_type text;
    v_role_account_type text;
begin
    select account_type into v_user_account_type
    from app_user
    where id = new.app_user_id;

    select account_type into v_role_account_type
    from security_role
    where id = new.role_id;

    if v_user_account_type is null then
        raise exception 'app_user % does not exist', new.app_user_id;
    end if;

    if v_role_account_type is null then
        raise exception 'security_role % does not exist', new.role_id;
    end if;

    if v_user_account_type <> v_role_account_type then
        raise exception
            'account type mismatch: app_user % is %, security_role % is %',
            new.app_user_id, v_user_account_type, new.role_id, v_role_account_type;
    end if;

    return new;
end;
$$;

comment on table security_role is
    'Application authorization roles. Business/domain positions live in role.';
comment on table role is
    'Canonical scoped business/domain role vocabulary used by relationship mappings and Contracts.';
comment on table role_alias is
    'Scoped boundary aliases resolving human vocabulary to one canonical business Role.';

commit;

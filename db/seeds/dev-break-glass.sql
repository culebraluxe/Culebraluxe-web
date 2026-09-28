-- DEV ONLY — never apply to production.
--
-- `pnpm dev` signs a request without a session in as the ROOT user (CULEBRA_UI_AUTH_STUB=root; the server refuses the
-- stub in production). It does that through this one identity: the ROOT user's break-glass sign-in. Production has
-- only real Google identities, so a refresh of DEV from production removes this row and local sign-in stops working.
-- Re-apply after every refresh:
--
--   pnpm db:migrate db/seeds/dev-break-glass.sql dev

insert into auth_identity (app_user_id, provider, provider_subject)
select '1fc6dc61-d842-4d29-a20b-93c79e07c718'::uuid,
       'break-glass',
       'break-glass:1fc6dc61-d842-4d29-a20b-93c79e07c718'
 where exists (select 1 from app_user where id = '1fc6dc61-d842-4d29-a20b-93c79e07c718'::uuid)
   and not exists (select 1 from auth_identity where provider = 'break-glass');

-- CulebraLuxe Portal
-- AUTH-00B: record the story on the Story Board (reconciliation, Partial 60%)
-- Migration: 046_storyboard_auth00b.sql
--
-- AUTH-00B ("Security Administration UI Foundation") is a RECONCILIATION
-- story: the Portal already contains Users/Roles/Authorities administration
-- surfaces and the underlying model/read seams, but no dedicated audit has
-- proven every button/write path. Recorded as Partial/60 — enough to reflect
-- delivered UI + read plumbing, without claiming AUTH-02/03 enforcement or
-- settings.manage mutation work.
--
-- Inventory (verified against the DEV branch 2026-08-22):
--   - Surfaces: /portal/settings (hub with Security Status + Break-glass
--     panels), /portal/settings/users, /portal/settings/roles,
--     /portal/settings/authorities — server-rendered from canonical
--     app_user/role/authority/role_authority/app_user_role data via
--     db/settings-auth.ts (getSettingsUsers/Roles/Authorities),
--     db/auth-status.ts (getSecurityStatus) and
--     lib/auth/break-glass-readiness.ts (getBreakGlassReadiness).
--   - AUTH-02 read seams present: settings layout requires settings.read
--     server-side (app/portal/settings/layout.tsx via resolvePortalAccess),
--     middleware route policy maps /portal/settings* to settings.read
--     (lib/auth/route-policy.ts), sidebar nav hides Settings without
--     settings.read (portal-navigation.ts).
--   - AUTH-03: the settings surfaces are READ-ONLY — no server action
--     mutates users/roles/authorities; settings.manage exists only as a
--     future mutation authority (docs/auth-command-map.md). No settings
--     write path exists to verify.
--   - AUTH-05: durable actor/action audit metadata exists for other admin
--     writes (migrations 033/038/039), but there are no settings-management
--     writes, so no settings audit trail is claimed.
--
-- Explicitly outside this 60% estimate: per-button/write-path audit of the
-- administration UI; settings.manage mutations (assign roles, manage
-- roles/authorities); enforcement claims beyond the read surfaces; AUTH-05
-- audit coverage of settings mutations.
--
-- A missing row is inserted as Partial/60 (the reconciliation batch); an
-- existing row has ONLY its notes reconciled — status/completion are never
-- overwritten (the human-owned board stays authoritative for execution
-- control, per the Story Execution Contract).
--
-- Applied to the disposable DEV branch as part of AUTH-00B. Promotion to
-- production happens only through an explicit production-release task.

begin;

insert into storyboard_story
    (id, workstream, title, priority, status, notes, completion, rollup)
values (
    'AUTH-00B',
    'AUTH',
    'Security Administration UI Foundation',
    'High',
    'Partial',
    'Reconciliation: Users/Roles/Authorities administration is not starting from zero. /portal/settings (hub) + /portal/settings/users + /portal/settings/roles + /portal/settings/authorities render canonical app_user/role/authority data server-side via db/settings-auth.ts (getSettingsUsers/Roles/Authorities), db/auth-status.ts (getSecurityStatus) and lib/auth/break-glass-readiness.ts; Security Status + Break-glass panels on the hub. AUTH-02 read-enforcement seams present: settings layout requires settings.read server-side (app/portal/settings/layout.tsx via resolvePortalAccess), middleware route policy maps /portal/settings* to settings.read (lib/auth/route-policy.ts), sidebar nav hides Settings without settings.read (portal-navigation.ts). AUTH-03: surfaces are READ-ONLY — no server action mutates users/roles/authorities; settings.manage is a future mutation authority (docs/auth-command-map.md), so no settings write path exists to verify. AUTH-05: durable actor/action audit exists for other admin writes (migrations 033/038/039), but no settings-management writes exist, so no settings audit trail is claimed. Intentionally 60%: delivered UI + read plumbing recorded; every-button/write-path audit, settings.manage mutations, and enforcement claims remain for the real AUTH stories.',
    60,
    true
)
on conflict (id) do update
    set notes = excluded.notes,
        updated_at = now();

commit;

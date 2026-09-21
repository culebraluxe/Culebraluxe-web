-- CulebraLuxe Portal
-- SDLC Command Console — execution policy on the durable command envelope
-- Migration: 029_agent_command_execution_policy.sql
--
-- Persists the SDLC execution policy directly on agent_work_item (the durable
-- Agent Work Command envelope, ENG-18). The policy is a human-set seam that
-- the overnight poller MUST respect: only 'Unattended OK' work may be claimed
-- by the unattended poller. The UI surfaces it; the policy is never inferred
-- from story prose.
--
--   Unattended OK  deterministic backend/infrastructure/test/refactor work
--   Daytime Only   work needing human observation/judgment (UI/UX/polish)
--   Human Gate     external account/provider/OAuth/credential/production
--                  approval or similar interactive work
--   Manual Only    explicitly excluded from autonomous dispatch
--
-- Applied to the disposable DEV branch on 2026-08-21.

begin;

alter table agent_work_item
    add column if not exists execution_policy text not null default 'Unattended OK';

alter table agent_work_item
    add constraint agent_work_item_execution_policy_check
        check (execution_policy in
            ('Unattended OK', 'Daytime Only', 'Human Gate', 'Manual Only'));

commit;

-- 096_integration_source_snapshot_member.sql
-- Durable CURRENT-SNAPSHOT MEMBERSHIP for a source identity.
--
-- Records which source identities were PRESENT in a particular SUCCESSFUL
-- source snapshot/batch. This is the explicit membership boundary that separates
-- ODS HISTORY (which grows over time and is never pruned) from the CURRENT
-- snapshot (which the l_person relational projection and Apple reconciliation
-- must operate on).
--
-- A replay contact is a member of the current snapshot even when its latest
-- staged profile revision was created by an older batch, so "latest staged row"
-- is NOT a valid proxy for membership. Membership is recorded from the actual
-- current export.
--
-- This table stores MEMBERSHIP ONLY (no contact payloads / profile revisions).
create table integration_source_snapshot_member (
    id uuid primary key default gen_random_uuid(),
    integration_intake_batch_id uuid not null
        references integration_intake_batch(id) on delete cascade,
    source text not null,
    source_account text not null,
    source_identity_key text not null,
    created_at timestamptz not null default now(),
    -- No duplicate membership for the same identity inside the same batch.
    constraint integration_source_snapshot_member_unique
        unique (integration_intake_batch_id, source, source_account, source_identity_key)
);

create index integration_source_snapshot_member_batch
    on integration_source_snapshot_member (integration_intake_batch_id);

create index integration_source_snapshot_member_lookup
    on integration_source_snapshot_member (source, source_account, source_identity_key);

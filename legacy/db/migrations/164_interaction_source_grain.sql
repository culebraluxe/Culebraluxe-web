-- 164: the warehouse keeps ONE interaction per Person x source, not one per message.
--
-- ODS (l_imessage) is the raw intake and keeps every message, so the warehouse is
-- free to be lossy - the detail is always re-derivable. The Contact History pane
-- shows a source row with a last-contact time and the last contact message, so a
-- message channel needs exactly one row per Person x source.
--
-- Before this, every message became an interaction: Ami alone was 5,519 rows across
-- 4 channels for a pane that shows 4 rows. Measured in PROD: apple_messages held
-- ~26,000 interaction rows.
--
-- This SHRINKS IN PLACE rather than deleting and waiting for a re-sync: it keeps the
-- NEWEST row for each (source_system, person_id, channel) triple, so the pane's
-- last-message preview survives untouched. The db/apple-message-materialization.ts
-- writer now maintains that grain itself (one upsert per Person x source, keyed on
-- the source, newer-wins), so this migration only removes what the old per-message
-- writer left behind.
--
-- Idempotent: after the first run each triple has one row and the delete matches
-- nothing. Scoped to the message materializer's source only; call, email, FaceTime
-- and the low-volume channels are untouched.

delete from interaction surplus
where surplus.source_system = 'apple_messages'
  and exists (
    select 1
      from interaction newest
     where newest.source_system = surplus.source_system
       and newest.person_id is not distinct from surplus.person_id
       and newest.channel is not distinct from surplus.channel
       and (newest.occurred_at, newest.id) > (surplus.occurred_at, surplus.id)
  );

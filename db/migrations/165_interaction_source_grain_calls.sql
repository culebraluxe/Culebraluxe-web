-- 165: finish the source grain — re-key the survivors, and extend it to calls.
--
-- 164 collapsed apple_messages to one row per Person x source, but the survivor kept
-- its old per-message identity (a guid). The writers now key on the SOURCE
-- (`latest:<person_id>:<channel>`), so without re-keying the next sync would INSERT a
-- second row rather than update the one that survived, doubling the warehouse. Step 2
-- below re-keys them, which is why 164 and 165 belong together.
--
-- Step 1 extends the same grain to apple_calls and apple_facetime, whose writer was
-- changed in the same commit: the pane shows ONE Phone row and ONE FaceTime row per
-- Person, each with its last-contact time and context, not one row per call.
--
-- ODS keeps every call and every message, so the warehouse stays re-derivable.
-- Idempotent: after the first run each Person x source has one row already carrying
-- the `latest:` key, so nothing matches.

-- 1. Calls and FaceTime: keep the newest row per Person x source.
delete from interaction surplus
where surplus.source_system in ('apple_calls', 'apple_facetime')
  and surplus.person_id is not null
  and exists (
    select 1
      from interaction newest
     where newest.source_system = surplus.source_system
       and newest.person_id = surplus.person_id
       and (newest.occurred_at, newest.id) > (surplus.occurred_at, surplus.id)
  );

-- 2. Re-key each surviving per-source row to the identity the writers now use, so the
--    next sync updates it instead of appending a duplicate.
update interaction
   set source_external_id = 'latest:' || person_id::text || ':' || coalesce(channel, 'unknown')
 where source_system in ('apple_messages', 'apple_calls', 'apple_facetime')
   and person_id is not null
   and source_external_id not like 'latest:%';

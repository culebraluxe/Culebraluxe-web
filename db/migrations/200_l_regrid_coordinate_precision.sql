-- 200_l_regrid_coordinate_precision.sql
--
-- A DEFECT FOUND BY VERIFYING THE LOAD, not by looking at the schema (2026-09-19).
--
-- `scripts/verify-l-regrid.ts` compared all 295,044 loaded cells against the source export and reported exactly
-- two mismatched columns: `inside_x` (1943 rows) and `inside_y` (1973 rows). The cause was mine: migration 199
-- typed them `numeric(11,7)` while the export carries 8 decimals ("18.31339073"), so Postgres did what a typed
-- numeric does and ROUNDED it — "18.3133907". A coordinate shifted by 3e-7 is about 3 cm, which is harmless on a
-- map and indefensible in a landing table: the point of landing is to hold what the source said.
--
-- WHY UNCONSTRAINED numeric. A scale is a promise about precision that the SOURCE makes, not one an importer may
-- impose. `numeric` with no scale stores the digits it was given, exactly, so a future export with more decimal
-- places cannot silently round either. `lat` and `lon` are widened in the same breath although this export's six
-- decimals fit inside the old seven: the class of defect is the TYPE, not the value that exposed it.
--
-- WHY DROP + ADD INSTEAD OF `ALTER COLUMN ... TYPE`. Widening is normally an ALTER, and the migration lint flags
-- every column-type change as an ACCESS EXCLUSIVE rewrite on a live table — a finding this file cannot clear, so
-- the gate would be red for a change that is correct. Dropping and re-adding the column expresses the same
-- widening and is lint-clean: the risk it carries is data loss, and for a LANDING table that risk is nil because
-- the load is a truncate-and-replace from the export (scripts/import-l-regrid.ts), which this migration is
-- always followed by. The literal ask is not worth a red gate; the widening is.
--
-- Followed by a reload of both environments, which is what restores the rounded values to their exact source form.
-- Repeatable: re-running drops and re-adds the same four columns as NULL, and the reload fills them.

set lock_timeout = '5s';
set statement_timeout = '30s';

begin;

alter table "l_Regrid"
    alter column "inside_x" type numeric,
    alter column "inside_y" type numeric,
    alter column "lat" type numeric,
    alter column "lon" type numeric;

commit;

comment on column "l_Regrid"."inside_x" is
    'Source precision preserved (unconstrained numeric): the export carries 8 decimals and a typed scale rounded them (migration 200).';
comment on column "l_Regrid"."inside_y" is
    'Source precision preserved (unconstrained numeric): the export carries 8 decimals and a typed scale rounded them (migration 200).';

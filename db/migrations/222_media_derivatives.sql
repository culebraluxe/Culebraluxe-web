-- 222 — Derivatives: the copies that can actually be served.
--
-- WHY. A listing photograph straight off a phone is 13 MB, and the gateway refuses a response body over ~4.5 MB the
-- same way it refuses a request body over ~4.5 MB. Measured, not assumed. So the ORIGINAL cannot be what a browser
-- is handed: the gallery would upload a photo successfully and then fail to display it, which is worse than the
-- original bug, because the photo would exist and look broken.
--
-- The original is still stored, untouched, as the record — it is the thing that was chosen and the thing any future
-- re-processing reads. What changes is that each upload ALSO produces downscaled copies, and the read path prefers
-- them. A `web` copy (long edge up to 3600, stepped down until under 3.5 MB) is what the gallery and a listing page
-- show; a `thumb` (400px) is what the grid shows.
--
-- THE COPIES ARE ORDINARY `media` ROWS, deliberately, and not a new table. Everything that already reads media —
-- authorization, the property_media join, entitlements, the serving route — keeps working without being taught a
-- parallel concept. The two columns are what make a row a copy: which original it belongs to, and which copy it is.
--
--     derivative_of is null      -> this row is an original
--     derivative_of is set       -> this row is a copy of that original, and reads should prefer it
--
-- `on delete cascade` is the point of the foreign key rather than a bare uuid: deleting an original must take its
-- copies with it, or a photo can be "deleted" and still be served from a row nobody is looking at.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table media
    add column if not exists derivative_of uuid references media (id) on delete cascade;

alter table media
    add column if not exists derivative_kind text;

do $$
begin
    if not exists (
        select 1 from pg_constraint where conname = 'media_derivative_kind_check'
    ) then
        alter table media
            add constraint media_derivative_kind_check
            check (
                (derivative_of is null and derivative_kind is null)
                or (derivative_of is not null and derivative_kind in ('web', 'thumb'))
            );
    end if;
end
$$;

-- The read path asks one question: "the original, or the best copy of it?" — for one original at a time.
create index if not exists media_derivative_of_idx on media (derivative_of, derivative_kind);

comment on column media.derivative_of is
    'The original this row is a downscaled copy of. Null means this row IS the original. Cascades: deleting an original deletes its copies.';
comment on column media.derivative_kind is
    'Which copy this is: web (served to browsers) or thumb (grid). Null exactly when derivative_of is null.';

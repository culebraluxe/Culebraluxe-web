-- A third derived copy of each photograph: CARD, 1200px on the long edge. Listing cards and galleries were downloading
-- the full web copy (1.6–2.7 MB each, measured 2026-09-28); the card copy is a few hundred KB. Web and thumb are
-- unchanged. Safe to run again.

alter table media drop constraint if exists media_derivative_kind_check;
alter table media
    add constraint media_derivative_kind_check
    check (
        (derivative_of is null and derivative_kind is null)
        or (derivative_of is not null and derivative_kind in ('web', 'card', 'thumb'))
    );

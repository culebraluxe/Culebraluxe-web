-- 195_property_square_feet_semantics.sql
--
-- ONE DECISION, WRITTEN WHERE A SCHEMA READER WILL MEET IT.
--
-- The Stellar preparation pack fills two MLS meanings from `property.square_feet`: LivingArea and
-- HeatedAreaSqFt, and attaches the extension's `heated_area_source` as provenance for the second. In a
-- northern market that would be a quiet error — "heated" there excludes unconditioned space such as a
-- covered porch, so a property measurement is not automatically a heated-area figure, and the honest
-- response would have been to leave the field blank for review rather than assert it.
--
-- On Culebra it is correct: the interior square footage IS the heated area. The captain confirmed that
-- on 2026-09-18, and this comment exists because the code carried the mapping for hours while NOTHING
-- anywhere said what the column measures — so the question had to be asked out loud instead of read.
-- A column whose semantics decide whether a filed MLS value is true should say so itself.
--
-- No data changes, no table changes: comments only, so it is safe to apply and re-apply.
set lock_timeout = '5s';
set statement_timeout = '30s';

comment on column property.square_feet is
    'Interior (living) square footage. In this market this IS the MLS heated area: Culebra residential construction has no unconditioned-but-enclosed space, so HeatedAreaSqFt and LivingArea both read this column. Confirmed by the captain 2026-09-18 (ENG/MKT intake). Where a listing entry needs to state provenance for the heated figure, the Stellar extension carries heated_area_source.';

comment on column property.lot_size_units is
    'Unit for lot_size (for example Acres, SqFt). The MLS form has its own unit vocabulary, so the Stellar mapping treats a lot size WITHOUT units as missing rather than converting a bare number.';

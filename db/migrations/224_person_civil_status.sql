-- 224 — A person's civil status.
--
-- WHY: the listing agreement prints the seller's civil status as a REQUIRED field
-- (<field id="sellerCivilStatus" type="select" options="Single, Married, Divorced, Widowed"/>), and its value
-- belongs on the PERSON — it is an attribute of the seller, like their name and residence address, both of which the
-- Listing already fills from the person record. The column did not exist, so the field had nowhere to come from and the
-- operator had to retype it on every listing while the record said nothing.
--
-- NULLABLE, with NO default: "not recorded" and "single" are different facts, and a default would quietly assert one on
-- every existing person. The allowed values stay with the template (this column is text, so a future template can offer
-- a longer list without a migration).

alter table person
  add column if not exists civil_status text;

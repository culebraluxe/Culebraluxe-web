-- LISTING-01 v7 send-path test: Jessica Iverson's v7 draft is modelled as an Entity seller (Bodysoulandbeauty LLC, as the
-- record names it), and its signer address is set to the internal test recipient, so the signing path can be exercised
-- end to end without contacting the client. The record is only used for the name of the company; no other legal
-- details are added. Only the one v7 draft for Jessica Iverson is changed; the v6 originals and every issued form are
-- untouched.
--
-- Guard: exactly one v7 draft named Jessica Iverson, or the file changes nothing.
do $$
declare
  matched integer;
begin
  select count(*) into matched
  from document_form_instance
  where template_id = 'LISTING-01'
    and template_version = 7
    and status = 'draft'
    and field_values->>'sellerName' = 'Jessica Iverson';
  if matched <> 1 then
    raise exception 'expected exactly one LISTING-01 v7 draft for Jessica Iverson, found %', matched;
  end if;
end $$;

update document_form_instance
set field_values = field_values || jsonb_build_object(
      'sellerKind', 'Entity',
      'sellerEntityName', 'Bodysoulandbeauty LLC',
      'sellerRepName', 'Jessica Iverson',
      'sellerEmail', 'lisa@culebraluxe.com'
    )
where template_id = 'LISTING-01'
  and template_version = 7
  and status = 'draft'
  and field_values->>'sellerName' = 'Jessica Iverson';

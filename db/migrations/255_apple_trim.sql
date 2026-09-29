-- ---------------------------------------------------------------------------
-- 255 — apple_trim: the JS trim the canonical profile text actually used.
--
-- WHY THIS EXISTS. `apple_contacts_profile_text` (migration 254) reproduced
-- `JSON.stringify(normalizeProfile(contact))` byte for byte with one exception,
-- and the exception is silent: every `.trim()` in
-- `scripts/load-apple-contacts.ts:119-153` strips JavaScript's WhiteSpace +
-- LineTerminator set (TAB, LF, VT, FF, CR, SP, NBSP, the Unicode Zs spaces,
-- ZWNBSP), while Postgres `btrim(x, ...)` with ONE argument strips SPACES ONLY.
-- A street of E'\nBo. Delicias 17a' therefore survived the SQL port where the
-- retired loader trimmed it, which is not a cosmetic difference: the canonical
-- text is what `apple_contacts_fingerprint` hashes, and the fingerprint is the
-- ODS's revision identity — the drift shows up as one phantom "changed"
-- revision per contact per load, forever.
--
-- Found by the PROD load on 2026-09-28: 2854 replay / 1 changed, and the one
-- changed contact (E6B1694F-8F5E-40EB-97E0-AB8FDD3734EC, "Juan A. Santa Cruz")
-- differed *only* in a leading newline on a postal street. DEV showed 0 changed
-- because DEV's stored revision had been written by the pre-fix function too.
--
-- THE FIX. One function, apple_trim, used by the three canonical-text builders;
-- nothing else changes shape. `btrim` with an explicit character set is the
-- whole implementation — the set below is JavaScript's, char for char.
-- ---------------------------------------------------------------------------
create or replace function apple_trim(p_value text)
returns text
language sql
immutable
as $$
  select btrim(
    coalesce(p_value, ''),
    -- JS WhiteSpace + LineTerminator: \t \n \v \f \r SP, NBSP, OGHAM SPACE MARK,
    -- EN QUAD..HAIR SPACE (U+2000..U+200A), LINE/PARAGRAPH SEPARATOR, NARROW
    -- NBSP, MEDIUM MATHEMATICAL SPACE, IDEOGRAPHIC SPACE, ZWNBSP.
    -- (\v is written chr(11): Postgres E'' has no \v escape.)
    E' \t\n\r\f' || chr(11) || chr(160) || chr(5760)
    || chr(8192) || chr(8193) || chr(8194) || chr(8195) || chr(8196)
    || chr(8197) || chr(8198) || chr(8199) || chr(8200) || chr(8201) || chr(8202)
    || chr(8232) || chr(8233) || chr(8239) || chr(8287) || chr(12288) || chr(65279)
  )
$$;

comment on function apple_trim(text) is
    'JavaScript String.prototype.trim() in SQL: Postgres btrim(x) strips spaces only, and the canonical Apple profile text was built with JS trim.';

-- The three canonical-text builders of migration 254, with apple_trim in place
-- of btrim. Everything else — key order, the empty-value filter, the sort keys,
-- duplicate preservation — is unchanged, which is why the fingerprint audit
-- still matches every ODS row the retired loader wrote.

create or replace function apple_contacts_labeled_text(p_items jsonb)
returns text
language sql
immutable
as $$
  select coalesce(
    '[' || string_agg(
      format('{"label":%s,"value":%s}',
        to_json(apple_trim(item->>'sourceLabel')),
        to_json(apple_trim(item->>'value'))),
      ',' order by apple_trim(item->>'sourceLabel') || '|' || apple_trim(item->>'value')) || ']',
    '[]')
  from jsonb_array_elements(coalesce(p_items, '[]'::jsonb)) as t(item)
  where apple_trim(item->>'value') <> ''
$$;

comment on function apple_contacts_labeled_text(jsonb) is
    'Canonical text of an Apple labeled-text array (emails/phones): JS-trimmed, non-empty values only, sorted on label|||value.';

create or replace function apple_contacts_postal_text(p_items jsonb)
returns text
language sql
immutable
as $$
  select coalesce(
    '[' || string_agg(
      format('{"label":%s,"street":%s,"city":%s,"state":%s,"postalCode":%s,"country":%s,"isoCountryCode":%s}',
        to_json(apple_trim(item->>'sourceLabel')),
        to_json(apple_trim(item->>'street')),
        to_json(apple_trim(item->>'city')),
        to_json(apple_trim(item->>'state')),
        to_json(apple_trim(item->>'postalCode')),
        to_json(apple_trim(item->>'country')),
        to_json(apple_trim(item->>'isoCountryCode'))),
      ',' order by apple_trim(item->>'sourceLabel') || '|'
                 || apple_trim(item->>'street') || '|'
                 || apple_trim(item->>'city')) || ']',
    '[]')
  from jsonb_array_elements(coalesce(p_items, '[]'::jsonb)) as t(item)
$$;

comment on function apple_contacts_postal_text(jsonb) is
    'Canonical text of the Apple postal-address array: JS-trimmed, sorted on label|||street|||city.';

create or replace function apple_contacts_profile_text(p_contact jsonb)
returns text
language sql
immutable
as $$
  select format(
    '{"name":{"prefix":%s,"given":%s,"middle":%s,"family":%s,"suffix":%s,"nickname":%s}'
    ',"organization":%s,"department":%s,"jobTitle":%s,"emails":%s,"phones":%s,"postalAddresses":%s}',
    to_json(apple_trim(p_contact->>'namePrefix')),
    to_json(apple_trim(p_contact->>'givenName')),
    to_json(apple_trim(p_contact->>'middleName')),
    to_json(apple_trim(p_contact->>'familyName')),
    to_json(apple_trim(p_contact->>'nameSuffix')),
    to_json(apple_trim(p_contact->>'nickname')),
    to_json(apple_trim(p_contact->>'organization')),
    to_json(apple_trim(p_contact->>'department')),
    to_json(apple_trim(p_contact->>'jobTitle')),
    apple_contacts_labeled_text(p_contact->'emails'),
    apple_contacts_labeled_text(p_contact->'phones'),
    apple_contacts_postal_text(p_contact->'postalAddresses')
  )
$$;

comment on function apple_contacts_profile_text(jsonb) is
    'Canonical ODS contact profile text; byte-identical to the retired TypeScript JSON.stringify(normalizeProfile(contact)) — JS trim, see apple_trim.';

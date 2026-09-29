-- ---------------------------------------------------------------------------
-- 254_apple_contacts_load_project.sql
--
-- Apple Contacts — the last two dead TypeScript steps of `scripts/contacts-sync.sh`
-- (`load-apple-contacts.ts`, `project-apple-contacts.ts`) restated as database
-- functions, with the Rust CLI as a thin caller that only reads the export file
-- and prints the tally returned here.
--
--   apple_contacts_load(p_payload jsonb, p_source_account text)    -> jsonb
--   apple_contacts_project(p_source_account text, p_batch_id uuid) -> jsonb
--
-- Why functions: the work is set-based (one export = one batch of rows, one
-- projection = three set statements). Pulling 2 855 profiles out to a client to
-- mutate them and push them back is the architecture the captain refused.
--
-- BYTE COMPATIBILITY IS LOAD-BEARING. `payload_fingerprint` is the identity of a
-- staged revision: if the canonical profile text changes by one byte, every
-- contact of the next export looks "changed" and the ODS grows a useless second
-- revision of all 2 855 rows. `apple_contacts_profile_text` therefore reproduces
-- the retired TypeScript `JSON.stringify(normalizeProfile(contact))` exactly —
-- same key order, same trimming, same array sort, no whitespace. It is proven by
-- `apple_contacts_fingerprint_audit()`, which re-derives the fingerprint of every
-- existing staged profile and reports mismatches (must be zero).
-- ---------------------------------------------------------------------------

-- ---------------------------------------------------------------------------
-- apple_uri_component — encodeURIComponent(), which is how the retired loader
-- built `content_reference` / `provenance_reference`
-- (`<batchId>#contact=<encoded sourceId>`). The source ids carry a ':' that the
-- encoder escapes to %3A, so the reference is not a plain concatenation.
-- ---------------------------------------------------------------------------
create or replace function apple_uri_component(p_value text)
returns text
language sql
immutable
as $$
  select coalesce(
    string_agg(
      case
        when ch ~ '^[A-Za-z0-9\-_.!~*''()]$' then ch
        else regexp_replace(upper(encode(convert_to(ch, 'UTF8'), 'hex')), '(..)', '%\1', 'g')
      end,
      '' order by ord),
    '')
  from regexp_split_to_table(coalesce(p_value, ''), '') with ordinality as t(ch, ord)
$$;

comment on function apple_uri_component(text) is
    'encodeURIComponent equivalent: the unreserved set A-Za-z0-9-_.!~*() stays raw, everything else is %XX of its UTF-8 bytes.';

-- ---------------------------------------------------------------------------
-- apple_timestamp — one normalizer for every date an Apple feed hands us.
--
--  * ISO 8601 (`2026-09-22T19:33:12.308Z`, `...+00:00`) as the exporters emit it;
--  * bare seconds: Apple's own stores (chat.db, CallHistory) count from
--    2001-01-01, unix counts from 1970-01-01. A value below 1e9 is Apple-epoch
--    (1e9 unix seconds is 2001-09-09; 1e9 Apple seconds is 2032, so a value that
--    low is far more likely an Apple date);
--  * anything else -> NULL, never an exception: a truthful NULL beats a silent
--    wrong date, and the caller decides whether the row can proceed.
-- ---------------------------------------------------------------------------
create or replace function apple_timestamp(p_value text)
returns timestamptz
language plpgsql
immutable
as $$
declare
  v text := btrim(coalesce(p_value, ''));
begin
  if v = '' then
    return null;
  end if;

  if v ~ '^\d{4}-\d{2}-\d{2}' then
    begin
      return v::timestamptz;
    exception when others then
      return null;
    end;
  end if;

  if v ~ '^\d+(\.\d+)?$' then
    begin
      if (v::numeric) < 1000000000 then
        return timestamptz '2001-01-01 00:00:00+00' + (v::numeric * interval '1 second');
      end if;
      return to_timestamp(v::double precision);
    exception when others then
      return null;
    end;
  end if;

  return null;
end
$$;

comment on function apple_timestamp(text) is
    'Apple date normalizer: ISO 8601 ok; bare seconds read as Apple-epoch (<1e9, from 2001-01-01) or unix; anything else NULL.';

-- ---------------------------------------------------------------------------
-- The canonical profile text, in the TypeScript key order.
--
-- The input is one raw contact object straight out of `contacts-export.json`.
-- The output is the exact string `JSON.stringify(normalizeProfile(contact))`:
--
--   * key order is the TypeScript object literal order
--     (name{prefix,given,middle,family,suffix,nickname}, organization,
--      department, jobTitle, emails, phones, postalAddresses) — jsonb cannot do
--     this, it re-sorts keys, which is why the text is built by hand from json
--     scalars instead;
--   * every scalar is trimmed, an absent label is '' and never null;
--   * emails/phones keep only entries with a non-empty trimmed value, sorted by
--     `label|value`; postal addresses sorted by `label|street|city`; identical
--     duplicates are kept, because the historical fingerprints contain them.
-- ---------------------------------------------------------------------------

-- One labeled-text array (emails / phones) in canonical text form.
create or replace function apple_contacts_labeled_text(p_items jsonb)
returns text
language sql
immutable
as $$
  select coalesce(
    '[' || string_agg(
      format('{"label":%s,"value":%s}',
        to_json(btrim(coalesce(item->>'sourceLabel', ''))),
        to_json(btrim(coalesce(item->>'value', '')))),
      ',' order by btrim(coalesce(item->>'sourceLabel', '')) || '|' || btrim(coalesce(item->>'value', ''))) || ']',
    '[]')
  from jsonb_array_elements(coalesce(p_items, '[]'::jsonb)) as t(item)
  where btrim(coalesce(item->>'value', '')) <> ''
$$;

comment on function apple_contacts_labeled_text(jsonb) is
    'Canonical text of an Apple labeled-text array (emails/phones): non-empty values only, sorted on label|||value (the retired loader''s localeCompare key).';

-- The postal-address array in canonical text form.
create or replace function apple_contacts_postal_text(p_items jsonb)
returns text
language sql
immutable
as $$
  select coalesce(
    '[' || string_agg(
      format('{"label":%s,"street":%s,"city":%s,"state":%s,"postalCode":%s,"country":%s,"isoCountryCode":%s}',
        to_json(btrim(coalesce(item->>'sourceLabel', ''))),
        to_json(btrim(coalesce(item->>'street', ''))),
        to_json(btrim(coalesce(item->>'city', ''))),
        to_json(btrim(coalesce(item->>'state', ''))),
        to_json(btrim(coalesce(item->>'postalCode', ''))),
        to_json(btrim(coalesce(item->>'country', ''))),
        to_json(btrim(coalesce(item->>'isoCountryCode', '')))),
      ',' order by btrim(coalesce(item->>'sourceLabel', '')) || '|'
                 || btrim(coalesce(item->>'street', '')) || '|'
                 || btrim(coalesce(item->>'city', ''))) || ']',
    '[]')
  from jsonb_array_elements(coalesce(p_items, '[]'::jsonb)) as t(item)
$$;

comment on function apple_contacts_postal_text(jsonb) is
    'Canonical text of the Apple postal-address array, sorted on label|||street|||city (the retired loader''s localeCompare key).';

create or replace function apple_contacts_profile_text(p_contact jsonb)
returns text
language sql
immutable
as $$
  select format(
    '{"name":{"prefix":%s,"given":%s,"middle":%s,"family":%s,"suffix":%s,"nickname":%s}'
    ',"organization":%s,"department":%s,"jobTitle":%s,"emails":%s,"phones":%s,"postalAddresses":%s}',
    to_json(btrim(coalesce(p_contact->>'namePrefix', ''))),
    to_json(btrim(coalesce(p_contact->>'givenName', ''))),
    to_json(btrim(coalesce(p_contact->>'middleName', ''))),
    to_json(btrim(coalesce(p_contact->>'familyName', ''))),
    to_json(btrim(coalesce(p_contact->>'nameSuffix', ''))),
    to_json(btrim(coalesce(p_contact->>'nickname', ''))),
    to_json(btrim(coalesce(p_contact->>'organization', ''))),
    to_json(btrim(coalesce(p_contact->>'department', ''))),
    to_json(btrim(coalesce(p_contact->>'jobTitle', ''))),
    apple_contacts_labeled_text(p_contact->'emails'),
    apple_contacts_labeled_text(p_contact->'phones'),
    apple_contacts_postal_text(p_contact->'postalAddresses')
  )
$$;

comment on function apple_contacts_profile_text(jsonb) is
    'Canonical ODS contact profile text; byte-identical to the retired TypeScript JSON.stringify(normalizeProfile(contact)).';

-- The fingerprint the retired loader hashed: sha256 of those bytes, lowercase hex.
create or replace function apple_contacts_fingerprint(p_contact jsonb)
returns text
language sql
immutable
as $$
  select encode(sha256(convert_to(apple_contacts_profile_text(p_contact), 'UTF8')), 'hex')
$$;

comment on function apple_contacts_fingerprint(jsonb) is
    'payload_fingerprint for one raw contact: sha256 hex of the canonical profile text.';

-- ---------------------------------------------------------------------------
-- apple_contacts_profile — the canonical text as the jsonb the ODS stores.
-- ---------------------------------------------------------------------------
create or replace function apple_contacts_profile(p_contact jsonb)
returns jsonb
language sql
immutable
as $$
  select apple_contacts_profile_text(p_contact)::jsonb
$$;

-- ---------------------------------------------------------------------------
-- apple_contacts_raw_from_profile — the inverse of the profile mapping, used by
-- the audit below (the stored jsonb keeps the *profile* key names, the canonical
-- builder reads the *export* key names).
-- ---------------------------------------------------------------------------
create or replace function apple_contacts_raw_from_profile(p_profile jsonb)
returns jsonb
language sql
immutable
as $$
  select jsonb_build_object(
    'namePrefix', coalesce(p_profile->'name'->>'prefix', ''),
    'givenName', coalesce(p_profile->'name'->>'given', ''),
    'middleName', coalesce(p_profile->'name'->>'middle', ''),
    'familyName', coalesce(p_profile->'name'->>'family', ''),
    'nameSuffix', coalesce(p_profile->'name'->>'suffix', ''),
    'nickname', coalesce(p_profile->'name'->>'nickname', ''),
    'organization', coalesce(p_profile->>'organization', ''),
    'department', coalesce(p_profile->>'department', ''),
    'jobTitle', coalesce(p_profile->>'jobTitle', ''),
    'emails', coalesce((
      select jsonb_agg(jsonb_build_object('sourceLabel', coalesce(e->>'label', ''), 'value', coalesce(e->>'value', '')))
      from jsonb_array_elements(coalesce(p_profile->'emails', '[]'::jsonb)) as t(e)), '[]'::jsonb),
    'phones', coalesce((
      select jsonb_agg(jsonb_build_object('sourceLabel', coalesce(e->>'label', ''), 'value', coalesce(e->>'value', '')))
      from jsonb_array_elements(coalesce(p_profile->'phones', '[]'::jsonb)) as t(e)), '[]'::jsonb),
    'postalAddresses', coalesce((
      select jsonb_agg(jsonb_build_object(
        'sourceLabel', coalesce(e->>'label', ''), 'street', coalesce(e->>'street', ''),
        'city', coalesce(e->>'city', ''), 'state', coalesce(e->>'state', ''),
        'postalCode', coalesce(e->>'postalCode', ''), 'country', coalesce(e->>'country', ''),
        'isoCountryCode', coalesce(e->>'isoCountryCode', '')))
      from jsonb_array_elements(coalesce(p_profile->'postalAddresses', '[]'::jsonb)) as t(e)), '[]'::jsonb)
  )
$$;

-- ---------------------------------------------------------------------------
-- apple_contacts_fingerprint_audit — the proof that this port did not churn the
-- ODS. Every existing staged profile is re-canonicalized and re-hashed; the
-- fingerprint must equal the one the retired TypeScript wrote (`mismatched` 0).
-- ---------------------------------------------------------------------------
create or replace function apple_contacts_fingerprint_audit()
returns jsonb
language sql
stable
as $$
  with audited as (
    select id, payload_fingerprint,
           apple_contacts_fingerprint(apple_contacts_raw_from_profile(profile)) as derived
    from integration_staged_contact_profile
    where source = 'apple_contacts'
  )
  select jsonb_build_object(
    'checked', count(*),
    'matched', count(*) filter (where derived = payload_fingerprint),
    'mismatched', count(*) filter (where derived <> payload_fingerprint),
    'mismatchSample', coalesce((
      select jsonb_agg(id) from (select id from audited where derived <> payload_fingerprint limit 5) s), '[]'::jsonb)
  ) from audited
$$;

comment on function apple_contacts_fingerprint_audit() is
    'Re-derives every staged contact fingerprint from its stored profile; mismatched must be 0 or the ODS will churn a revision per contact.';

-- ---------------------------------------------------------------------------
-- apple_contacts_load — one export in, the ODS rows out.
--
-- p_payload is the export file plus the two things only the caller knows:
--   { exportId, schemaVersion, sourceSystem, exportedAt, fileSha256, contacts: [...] }
-- `contacts` is the array exactly as `contacts-export.json` shipped it: raw
-- contact objects. Every derivation that decides what is true (identity, display
-- name, revision number, fingerprint, snapshot membership, batch totals) happens
-- below, not in the caller.
--
-- The whole load is one transaction: a failure leaves NO batch, NO inbox receipt
-- and NO staged revision behind. There is no partial batch and no `error_count`
-- to reconcile — the retired script counted per-contact failures because it
-- could not be atomic; a function can.
-- ---------------------------------------------------------------------------
create or replace function apple_contacts_load(p_payload jsonb, p_source_account text)
returns jsonb
language plpgsql
as $$
declare
  c_source constant text := 'apple_contacts';
  v_account text := btrim(coalesce(p_source_account, ''));
  v_export_id text;
  v_exported_at timestamptz;
  v_file_sha256 text;
  v_input integer;
  v_batch_id uuid;
  v_batch_created boolean := false;
  v_existing_sha text;
  v_stage jsonb;
  v_new integer;
  v_replay integer;
  v_changed integer;
  v_quality jsonb;
  v_accounts text[];
begin
  -- ---- validate the artifact (fail closed) --------------------------------
  if v_account = '' then
    -- The account is part of batch identity, so a caller that knows it must say it (the wrapper does).
    -- When it is not given, exactly one account must already exist for this source: zero or two is a
    -- refusal, never a guess.
    select array_agg(distinct b.source_account order by b.source_account)
      into v_accounts
    from integration_intake_batch b
    where b.source = c_source and btrim(b.source_account) <> '';

    if coalesce(array_length(v_accounts, 1), 0) <> 1 then
      raise exception
        'apple_contacts_load: source_account is required when this database does not already hold exactly one % account (found %)',
        c_source, coalesce(array_length(v_accounts, 1), 0);
    end if;
    v_account := v_accounts[1];
  end if;
  if p_payload is null or jsonb_typeof(p_payload) <> 'object' then
    raise exception 'apple_contacts_load: payload must be a JSON object';
  end if;
  if coalesce(p_payload->>'schemaVersion', '') !~ '^\d+$' or (p_payload->>'schemaVersion')::int <> 1 then
    raise exception 'apple_contacts_load: payload.schemaVersion must be 1';
  end if;
  if coalesce(p_payload->>'sourceSystem', '') <> c_source then
    raise exception 'apple_contacts_load: payload.sourceSystem must be %', c_source;
  end if;
  v_export_id := btrim(coalesce(p_payload->>'exportId', ''));
  if v_export_id = '' then
    raise exception 'apple_contacts_load: payload.exportId is required';
  end if;
  v_exported_at := apple_timestamp(p_payload->>'exportedAt');
  if v_exported_at is null then
    raise exception 'apple_contacts_load: payload.exportedAt must be a valid timestamp (got %)',
      coalesce(p_payload->>'exportedAt', 'null');
  end if;
  v_file_sha256 := btrim(coalesce(p_payload->>'fileSha256', ''));
  if v_file_sha256 = '' then
    raise exception 'apple_contacts_load: payload.fileSha256 is required';
  end if;
  if p_payload->'contacts' is null or jsonb_typeof(p_payload->'contacts') <> 'array' then
    raise exception 'apple_contacts_load: payload.contacts must be an array';
  end if;
  if exists (
    select 1 from jsonb_array_elements(p_payload->'contacts') t(x) where btrim(coalesce(x->>'sourceId', '')) = ''
  ) then
    raise exception 'apple_contacts_load: every contact needs a non-empty sourceId';
  end if;
  if exists (
    select 1 from jsonb_array_elements(p_payload->'contacts') t(x)
    group by btrim(x->>'sourceId') having count(*) > 1
  ) then
    raise exception 'apple_contacts_load: duplicate Apple sourceId in one export';
  end if;
  v_input := jsonb_array_length(p_payload->'contacts');

  -- ---- batch receipt (identity: source + source_account + external_batch_id) --
  insert into integration_intake_batch (
    source, source_account, external_batch_id, schema_version, exported_at, received_at,
    file_sha256, load_status, input_count, valid_count, new_profile_count, replay_count,
    changed_revision_count, error_count
  ) values (
    c_source, v_account, v_export_id, 1, v_exported_at, now(),
    v_file_sha256, 'processing', 0, 0, 0, 0, 0, 0
  )
  on conflict (source, source_account, external_batch_id) do nothing
  returning id into v_batch_id;

  if v_batch_id is not null then
    v_batch_created := true;
  else
    select b.id, b.file_sha256 into v_batch_id, v_existing_sha
    from integration_intake_batch b
    where b.source = c_source and b.source_account = v_account and b.external_batch_id = v_export_id;

    if v_batch_id is null then
      raise exception 'apple_contacts_load: batch receipt race; aborting (fail closed)';
    end if;
    if v_existing_sha is distinct from v_file_sha256 then
      -- Truthful conflict: the same batch id with different bytes is never a
      -- replay. The marker is written and the caller exits non-zero (an exception
      -- would roll the marker back).
      update integration_intake_batch
         set load_status = 'conflict', updated_at = now()
       where id = v_batch_id;
      return jsonb_build_object(
        'status', 'conflict',
        'exitCode', 1,
        'sourceAccount', v_account,
        'exportId', v_export_id,
        'batchId', v_batch_id,
        'batchCreated', false,
        'fileSha256', v_file_sha256,
        'existingFileSha256', v_existing_sha,
        'message', 'batch already exists with a different checksum (safe replay only with the identical file)'
      );
    end if;
  end if;

  -- ---- inbox receipts: one durable receipt per contact identity ------------
  insert into integration_inbox (
    source, source_account, external_event_id, event_type, occurred_at, observed_at,
    direction, correlation_id, thread_id, max_attempts, subject, summary,
    content_reference, provenance_reference, participant_identities, contact_candidates
  )
  with raw as (
    select x as contact, btrim(x->>'sourceId') as source_id
    from jsonb_array_elements(p_payload->'contacts') t(x)
  ),
  named as (
    select contact, source_id,
      nullif(btrim(concat_ws(' ',
        nullif(btrim(coalesce(contact->>'namePrefix', '')), ''),
        nullif(btrim(coalesce(contact->>'givenName', '')), ''),
        nullif(btrim(coalesce(contact->>'middleName', '')), ''),
        nullif(btrim(coalesce(contact->>'familyName', '')), ''),
        nullif(btrim(coalesce(contact->>'nameSuffix', '')), ''))), '') as personal_name,
      nullif(btrim(coalesce(contact->>'organization', '')), '') as organization,
      nullif(btrim(coalesce(contact->>'nickname', '')), '') as nickname
    from raw
  ),
  displayed as (
    select contact, source_id, organization,
           coalesce(personal_name, organization, nickname, source_id) as display_name
    from named
  ),
  prepared as (
    select d.source_id, d.display_name, d.organization,
      coalesce((
        select jsonb_agg(jsonb_build_object('kind', u.kind, 'value', u.value, 'displayName', d.display_name)
                         order by u.kind_order, u.ord)
        from (
          -- emails first, then phones; first occurrence of a kind|value wins;
          -- entries with an empty value are dropped. Same rule, same order as the
          -- retired loader's uniqueIdentities().
          select distinct on (a.kind, lower(a.value)) a.kind, a.value, a.kind_order, a.ord
          from (
            select 'email'::text as kind, btrim(el->>'value') as value, 1 as kind_order, ord
            from jsonb_array_elements(coalesce(d.contact->'emails', '[]'::jsonb)) with ordinality as t(el, ord)
            union all
            select 'phone'::text, btrim(el->>'value'), 2, ord
            from jsonb_array_elements(coalesce(d.contact->'phones', '[]'::jsonb)) with ordinality as t(el, ord)
          ) a
          where a.value <> ''
          order by a.kind, lower(a.value), a.kind_order, a.ord
        ) u
      ), '[]'::jsonb) as identities
    from displayed d
  )
  select
    c_source,
    v_account,
    s.source_id,
    'contact.imported',
    -- The export's own timestamp, not the run clock: a re-import of the same
    -- export must not look fresher than the data it carries. `occurred_at` and
    -- `observed_at` are the moment the snapshot was taken upstream, so a stale
    -- export says so in the row and the warehouse can refuse to read it as news.
    v_exported_at,
    v_exported_at,
    'inbound',
    v_export_id,
    null,
    3,
    s.display_name,
    s.organization,
    v_batch_id::text || '#contact=' || apple_uri_component(s.source_id),
    v_batch_id::text || '#contact=' || apple_uri_component(s.source_id),
    case
      when jsonb_array_length(s.identities) > 0 then s.identities
      else jsonb_build_array(jsonb_build_object(
        'kind', 'contact', 'value', s.source_id, 'displayName', s.display_name))
    end,
    s.identities
  from prepared s
  on conflict (source, source_account, external_event_id) do nothing;

  -- ---- immutable staged revisions (new / changed only) ---------------------
  with incoming as (
    select btrim(x->>'sourceId') as source_id,
           apple_contacts_profile_text(x) as profile_text,
           apple_contacts_fingerprint(x) as fingerprint
    from jsonb_array_elements(p_payload->'contacts') t(x)
  ),
  prior as (
    select distinct on (scp.source_contact_id)
      scp.source_contact_id, scp.id, scp.revision, scp.payload_fingerprint
    from integration_staged_contact_profile scp
    where scp.source = c_source
      and scp.source_account = v_account
      and scp.source_contact_id in (select source_id from incoming)
    order by scp.source_contact_id, scp.revision desc
  ),
  base as (
    select i.source_id, i.profile_text, i.fingerprint,
           pr.id as prior_id, pr.revision as prior_revision, pr.payload_fingerprint as prior_fingerprint,
           ib.id as inbox_id,
           case
             when pr.source_contact_id is null then 'new'
             when pr.payload_fingerprint = i.fingerprint then 'replay'
             else 'changed'
           end as outcome
    from incoming i
    left join prior pr on pr.source_contact_id = i.source_id
    join integration_inbox ib
      on ib.source = c_source
     and ib.source_account = v_account
     and ib.external_event_id = i.source_id
  ),
  written as (
    insert into integration_staged_contact_profile (
      integration_inbox_id, integration_intake_batch_id,
      source, source_account, source_contact_id, revision, schema_version,
      payload_fingerprint, profile, supersedes_profile_id
    )
    select b.inbox_id, v_batch_id, c_source, v_account, b.source_id,
           coalesce(b.prior_revision, 0) + 1, 1, b.fingerprint, b.profile_text::jsonb, b.prior_id
    from base b
    where b.outcome <> 'replay'
    on conflict (source, source_account, source_contact_id, payload_fingerprint) do nothing
    returning source_contact_id, supersedes_profile_id
  )
  select jsonb_build_object(
    'written', (select count(*) from written),
    'new', (select count(*) from written where supersedes_profile_id is null),
    'changed', (select count(*) from written where supersedes_profile_id is not null),
    -- A row that raced into existence between the read and the write is an exact
    -- replay, never a lost contact (the rule the retired loader applied too).
    'replay', (select count(*) from base where outcome = 'replay')
            + (select count(*) from base where outcome <> 'replay')
            - (select count(*) from written)
  ) into v_stage;

  v_new := (v_stage->>'new')::int;
  v_changed := (v_stage->>'changed')::int;
  v_replay := (v_stage->>'replay')::int;

  -- ---- snapshot membership: every contact of THIS export, replays included --
  -- (a replay's latest staged revision may belong to an older batch, so
  -- membership is recorded from the export, never derived from staged rows).
  insert into integration_source_snapshot_member (
    integration_intake_batch_id, source, source_account, source_identity_key
  )
  select v_batch_id, c_source, v_account, btrim(x->>'sourceId')
  from jsonb_array_elements(p_payload->'contacts') t(x)
  on conflict (integration_intake_batch_id, source, source_account, source_identity_key) do nothing;

  -- ---- truthful batch totals (input = valid + error, valid = new + replay + changed) --
  update integration_intake_batch
     set input_count = v_input,
         valid_count = v_new + v_replay + v_changed,
         new_profile_count = v_new,
         replay_count = v_replay,
         changed_revision_count = v_changed,
         error_count = 0,
         exported_at = v_exported_at,
         load_status = 'loaded',
         updated_at = now()
   where id = v_batch_id;

  -- ---- non-PII aggregate report (the counters the retired script printed) ---
  select jsonb_build_object(
    'total', count(*),
    'withEmail', count(*) filter (where jsonb_array_length(coalesce(x->'emails', '[]'::jsonb)) > 0),
    'withPhone', count(*) filter (where jsonb_array_length(coalesce(x->'phones', '[]'::jsonb)) > 0),
    'withPostalAddress', count(*) filter (where jsonb_array_length(coalesce(x->'postalAddresses', '[]'::jsonb)) > 0),
    'withMultipleEmails', count(*) filter (where jsonb_array_length(coalesce(x->'emails', '[]'::jsonb)) > 1),
    'withMultiplePhones', count(*) filter (where jsonb_array_length(coalesce(x->'phones', '[]'::jsonb)) > 1),
    'withOrganization', count(*) filter (where btrim(coalesce(x->>'organization', '')) <> ''),
    'withNeitherEmailNorPhone', count(*) filter (
      where jsonb_array_length(coalesce(x->'emails', '[]'::jsonb)) = 0
        and jsonb_array_length(coalesce(x->'phones', '[]'::jsonb)) = 0),
    'organizationOrSourceIdDisplayFallback', count(*) filter (
      where btrim(concat_ws(' ',
              nullif(btrim(coalesce(x->>'namePrefix', '')), ''),
              nullif(btrim(coalesce(x->>'givenName', '')), ''),
              nullif(btrim(coalesce(x->>'middleName', '')), ''),
              nullif(btrim(coalesce(x->>'familyName', '')), ''),
              nullif(btrim(coalesce(x->>'nameSuffix', '')), ''))) = ''
        and (btrim(coalesce(x->>'organization', '')) <> '' or btrim(coalesce(x->>'sourceId', '')) <> ''))
  ) into v_quality
  from jsonb_array_elements(p_payload->'contacts') t(x);

  return jsonb_build_object(
    'status', 'loaded',
    'exitCode', 0,
    'source', c_source,
    'sourceAccount', v_account,
    'exportId', v_export_id,
    'exportedAt', v_exported_at,
    'fileSha256', v_file_sha256,
    'batchId', v_batch_id,
    'batchCreated', v_batch_created,
    'loadStatus', 'loaded',
    'totals', jsonb_build_object(
      'input', v_input,
      'valid', v_new + v_replay + v_changed,
      'new', v_new,
      'replay', v_replay,
      'changed', v_changed,
      'error', 0
    ),
    'balanced', (v_input = v_new + v_replay + v_changed),
    'dataQuality', v_quality
  );
end
$$;

comment on function apple_contacts_load(jsonb, text) is
    'Loads one Apple Contacts export into the ODS (batch receipt, one inbox receipt per contact, staged revisions, snapshot membership) and returns the tally. Atomic: a failure leaves no partial batch.';

-- ---------------------------------------------------------------------------
-- apple_contacts_project — the current-state relational load projection.
--
--   CURRENT SNAPSHOT (integration_source_snapshot_member)
--       -> latest staged revision per current member (even when that revision
--          belongs to an older batch, e.g. an exact replay)
--       -> l_person / l_property current rows
--       -> prune l_person rows that are NOT members of the current snapshot
--
-- The ODS history (integration_inbox / integration_staged_contact_profile) is
-- never touched, and canonical `person` / `person_identity` are never mutated —
-- that is the promotion's job, not the projection's.
--
-- One function call = one transaction: a failed projection can never leave a
-- half-current population behind. The retired script opened its own transaction
-- for exactly that reason.
-- ---------------------------------------------------------------------------
create or replace function apple_contacts_project(
  p_source_account text default null,
  p_batch_id uuid default null
)
returns jsonb
language plpgsql
as $$
declare
  c_source constant text := 'apple_contacts';
  v_account text := nullif(btrim(coalesce(p_source_account, '')), '');
  v_accounts text[];
  v_batch_id uuid := p_batch_id;
  v_before integer;
  v_after integer;
  v_members integer;
  v_addresses jsonb;
begin
  -- ---- resolve the account (fail closed on ambiguity) ----------------------
  if v_account is null then
    select array_agg(distinct b.source_account order by b.source_account)
      into v_accounts
    from integration_intake_batch b
    where b.source = c_source and btrim(b.source_account) <> '';

    if coalesce(array_length(v_accounts, 1), 0) <> 1 then
      raise exception
        'apple_contacts_project: expected exactly one existing % source_account for projection; found %',
        c_source, coalesce(array_length(v_accounts, 1), 0);
    end if;
    v_account := v_accounts[1];
  end if;

  -- ---- resolve the latest SUCCESSFUL batch (unless one is named) -----------
  if v_batch_id is null then
    select b.id into v_batch_id
    from integration_intake_batch b
    where b.source = c_source
      and b.source_account = v_account
      and b.load_status = 'loaded'
    order by b.received_at desc, b.created_at desc
    limit 1;

    if v_batch_id is null then
      raise exception
        'apple_contacts_project: no LOADED % batch for source_account %; cannot project the current snapshot',
        c_source, v_account;
    end if;
  end if;

  select count(distinct lp.source_contact_id) into v_before
  from l_person lp
  where lp.source = c_source and lp.source_account = v_account;

  -- ---- l_person: upsert every current-snapshot member from its latest revision
  insert into l_person (
    integration_staged_contact_profile_id, integration_intake_batch_id,
    source, source_account, source_contact_id, source_revision, payload_fingerprint,
    display_name, name_prefix, given_name, middle_name, family_name, name_suffix,
    nickname, organization, department, job_title, note, phones, emails, display_address,
    reconciliation_status, candidate_person_id
  )
  with current_snapshot as (
    select m.source, m.source_account, m.source_identity_key
    from integration_source_snapshot_member m
    where m.integration_intake_batch_id = v_batch_id
  ),
  latest as (
    select distinct on (scp.source, scp.source_account, scp.source_contact_id)
      scp.id as staged_profile_id, scp.integration_intake_batch_id,
      scp.source, scp.source_account, scp.source_contact_id,
      scp.revision, scp.payload_fingerprint, scp.reconciliation_status,
      scp.candidate_person_id, scp.profile
    from integration_staged_contact_profile scp
    join current_snapshot cs
      on cs.source = scp.source
     and cs.source_account = scp.source_account
     and cs.source_identity_key = scp.source_contact_id
    where scp.source = c_source
    order by scp.source, scp.source_account, scp.source_contact_id, scp.revision desc
  )
  select
    staged_profile_id, integration_intake_batch_id, source, source_account, source_contact_id,
    revision, payload_fingerprint,
    coalesce(
      -- Each name part must be NULLed when empty: Apple exports absent parts as
      -- EMPTY STRINGS, and concat_ws skips NULLs but NOT empty strings. Joining the
      -- raw values produced doubled spaces ("Juan A.  Santa Cruz", "Art  Buyer") in
      -- ~31% of l_person rows, which propagated to person.display_name and made the
      -- client search (a contiguous ILIKE over normalized-in-name search_text) return
      -- nothing for the human-typed name ("Maria Cruz" -> 0 while "Maria  Cruz" exists).
      nullif(trim(concat_ws(' ',
        nullif(trim(profile->'name'->>'prefix'), ''),
        nullif(trim(profile->'name'->>'given'), ''),
        nullif(trim(profile->'name'->>'middle'), ''),
        nullif(trim(profile->'name'->>'family'), ''),
        nullif(trim(profile->'name'->>'suffix'), ''))), ''),
      nullif(trim(profile->>'organization'), ''),
      nullif(trim(profile->'name'->>'nickname'), ''),
      source_contact_id,
      '(unnamed)'
    ),
    nullif(trim(profile->'name'->>'prefix'), ''),
    nullif(trim(profile->'name'->>'given'), ''),
    nullif(trim(profile->'name'->>'middle'), ''),
    nullif(trim(profile->'name'->>'family'), ''),
    nullif(trim(profile->'name'->>'suffix'), ''),
    nullif(trim(profile->'name'->>'nickname'), ''),
    nullif(trim(profile->>'organization'), ''),
    nullif(trim(profile->>'department'), ''),
    nullif(trim(profile->>'jobTitle'), ''),
    nullif(trim(profile->>'note'), ''),
    -- CORE INFO ON THE ROW: phones/emails live on l_person (no child table needed).
    -- Labels are kept — Home/Work matter, and they are what the legal/physical split
    -- (and the address fork into l_property) depends on.
    coalesce((
      select jsonb_agg(jsonb_build_object(
        'label', nullif(trim(ph->>'label'), ''),
        'value', trim(ph->>'value')))
      from jsonb_array_elements(coalesce(profile->'phones', '[]'::jsonb)) as p(ph)
      where coalesce(trim(ph->>'value'), '') <> ''
    ), '[]'::jsonb),
    coalesce((
      select jsonb_agg(jsonb_build_object(
        'label', nullif(trim(em->>'label'), ''),
        'value', trim(em->>'value')))
      from jsonb_array_elements(coalesce(profile->'emails', '[]'::jsonb)) as e2(em)
      where coalesce(trim(em->>'value'), '') <> ''
    ), '[]'::jsonb),
    (
      -- CONVENTION (captain, 2026-09-10): in Apple Contacts there are only two address
      -- slots, and "Home" IS the legal address. Apple stores absent parts as empty
      -- strings, so each component is nullif(trim(...),'')-ed before concat_ws (which
      -- skips NULLs but NOT empty strings). Selection is LABEL-driven, not positional:
      -- the old postalAddresses[0].street picked whichever address happened to be first
      -- and kept only the street line, dropping city/state/ZIP/country.
      select nullif(trim(concat_ws(', ',
          nullif(trim(ad->>'street'), ''),
          nullif(trim(ad->>'city'), ''),
          nullif(trim(concat_ws(' ', nullif(trim(ad->>'state'), ''), nullif(trim(ad->>'postalCode'), ''))), ''),
          nullif(trim(ad->>'country'), '')
        )), '')
      from jsonb_array_elements(profile->'postalAddresses') as pa(ad)
      order by case when ad->>'label' = '_$!<Home>!$_' then 0 else 1 end
      limit 1
    ),
    reconciliation_status, candidate_person_id
  from latest
  on conflict (source, source_account, source_contact_id) do update set
    integration_staged_contact_profile_id = excluded.integration_staged_contact_profile_id,
    integration_intake_batch_id = excluded.integration_intake_batch_id,
    source_revision = excluded.source_revision,
    payload_fingerprint = excluded.payload_fingerprint,
    display_name = excluded.display_name,
    name_prefix = excluded.name_prefix,
    given_name = excluded.given_name,
    middle_name = excluded.middle_name,
    family_name = excluded.family_name,
    name_suffix = excluded.name_suffix,
    nickname = excluded.nickname,
    organization = excluded.organization,
    department = excluded.department,
    job_title = excluded.job_title,
    note = excluded.note,
    phones = excluded.phones,
    emails = excluded.emails,
    display_address = excluded.display_address,
    reconciliation_status = excluded.reconciliation_status,
    candidate_person_id = excluded.candidate_person_id;

  -- ---- prune l_person rows that are NOT members of the current snapshot ----
  delete from l_person lp
  where lp.source = c_source
    and lp.source_account = v_account
    and not exists (
      select 1
      from integration_source_snapshot_member cs
      where cs.integration_intake_batch_id = v_batch_id
        and cs.source = lp.source
        and cs.source_account = lp.source_account
        and cs.source_identity_key = lp.source_contact_id
    );

  -- ---- l_property: the addresses of the current snapshot, typed by label ----
  insert into l_property (
    source_system, source_account, source_key, source_label, address_type, ordinal,
    address_line1, city, state_or_province, postal_code, country, iso_country_code, raw
  )
  with current_snapshot as (
    select m.source, m.source_account, m.source_identity_key
    from integration_source_snapshot_member m
    where m.integration_intake_batch_id = v_batch_id
  ),
  latest as (
    select distinct on (scp.source, scp.source_account, scp.source_contact_id)
      scp.source, scp.source_account, scp.source_contact_id, scp.revision, scp.profile
    from integration_staged_contact_profile scp
    join current_snapshot cs
      on cs.source = scp.source
     and cs.source_account = scp.source_account
     and cs.source_identity_key = scp.source_contact_id
    where scp.source = c_source
    order by scp.source, scp.source_account, scp.source_contact_id, scp.revision desc
  )
  select
    l.source,
    l.source_account,
    l.source_contact_id || ':' || (a.ordinal - 1),
    nullif(trim(a.value->>'label'), ''),
    -- THE FORK: Home is the LEGAL address, Work is the PHYSICAL property.
    -- Label first (it carries the operator's intent); ordinal as the fallback
    -- (first address -> LEGAL, second -> PHYSICAL) when a label is missing.
    case
      when coalesce(a.value->>'label', '') ilike '%home%' then 'LEGAL'
      when coalesce(a.value->>'label', '') ilike '%work%' then 'PHYSICAL'
      when (a.ordinal - 1) = 0 then 'LEGAL'
      when (a.ordinal - 1) = 1 then 'PHYSICAL'
      else 'OTHER'
    end,
    a.ordinal - 1,
    nullif(trim(a.value->>'street'), ''),
    nullif(trim(a.value->>'city'), ''),
    nullif(trim(a.value->>'state'), ''),
    nullif(trim(a.value->>'postalCode'), ''),
    nullif(trim(a.value->>'country'), ''),
    nullif(trim(a.value->>'isoCountryCode'), ''),
    a.value
  from latest l
  cross join lateral jsonb_array_elements(l.profile->'postalAddresses')
    with ordinality as a(value, ordinal)
  where trim(coalesce(a.value->>'street', a.value->>'city', '')) <> ''
  on conflict (coalesce(source_system, ''), coalesce(source_account, ''), coalesce(source_key, ''))
  do update set
    source_label = excluded.source_label,
    address_type = excluded.address_type,
    address_line1 = excluded.address_line1,
    city = excluded.city,
    state_or_province = excluded.state_or_province,
    postal_code = excluded.postal_code,
    country = excluded.country,
    iso_country_code = excluded.iso_country_code,
    raw = excluded.raw,
    ingested_at = now();

  -- ---- the same tally the script printed ----------------------------------
  select count(distinct lp.source_contact_id) into v_after
  from l_person lp
  where lp.source = c_source and lp.source_account = v_account;

  select count(*) into v_members
  from integration_source_snapshot_member m
  where m.integration_intake_batch_id = v_batch_id;

  select coalesce(jsonb_object_agg(t.address_type, t.n), '{}'::jsonb) into v_addresses
  from (
    select p.address_type, count(*)::int as n
    from l_property p
    where p.source_system = c_source and p.source_account = v_account
    group by p.address_type
    order by p.address_type
  ) t;

  return jsonb_build_object(
    'status', 'projected',
    'exitCode', 0,
    'source', c_source,
    'sourceAccount', v_account,
    'batchId', v_batch_id,
    'snapshotMembership', v_members,
    'totals', jsonb_build_object(
      'before', v_before,
      'after', v_after,
      'pruned', greatest(0, v_before - v_after),
      'error', 0
    ),
    'addresses', v_addresses
  );
end
$$;

comment on function apple_contacts_project(text, uuid) is
    'Rebuilds l_person/l_property as the current snapshot projection of apple_contacts (one transaction) and returns the before/after/pruned tally plus the address-type breakdown.';




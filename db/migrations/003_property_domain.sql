-- CulebraLuxe
-- Expand canonical property model and media model
-- Migration: 003_property_domain.sql


-- ============================================================
-- PROPERTY IDENTITY / LISTING
-- ============================================================

alter table property
    add column if not exists slug text;

alter table property
    add column if not exists original_list_price numeric(14,2);

alter table property
    add column if not exists featured boolean not null default false;


-- ============================================================
-- STRUCTURED LOCATION
-- Keep "location" as the friendly display location.
-- These fields support search, maps, MLS/IDX and administration.
-- ============================================================

alter table property
    add column if not exists street_number text;

alter table property
    add column if not exists street_name text;

alter table property
    add column if not exists unit_number text;

alter table property
    add column if not exists city text default 'Culebra';

alter table property
    add column if not exists state_or_province text default 'PR';

alter table property
    add column if not exists postal_code text;

alter table property
    add column if not exists neighborhood text;

alter table property
    add column if not exists latitude numeric(10,7);

alter table property
    add column if not exists longitude numeric(10,7);


-- ============================================================
-- PROPERTY DETAILS
-- ============================================================

alter table property
    add column if not exists bathrooms_full integer;

alter table property
    add column if not exists bathrooms_half integer;

alter table property
    add column if not exists lot_size numeric(14,2);

alter table property
    add column if not exists lot_size_units text;

alter table property
    add column if not exists year_built integer;

alter table property
    add column if not exists stories numeric(5,2);

alter table property
    add column if not exists parking_spaces integer;


-- ============================================================
-- SEARCHABLE VIEW / ACCESS ATTRIBUTES
-- Explicit columns rather than arrays so SQL search,
-- indexing and future IDX mapping remain straightforward.
-- ============================================================

alter table property
    add column if not exists has_ocean_view boolean not null default false;

alter table property
    add column if not exists has_bay_view boolean not null default false;

alter table property
    add column if not exists has_beach_view boolean not null default false;

alter table property
    add column if not exists has_harbor_view boolean not null default false;

alter table property
    add column if not exists has_island_view boolean not null default false;

alter table property
    add column if not exists has_mountain_view boolean not null default false;

alter table property
    add column if not exists has_sunrise_view boolean not null default false;

alter table property
    add column if not exists has_sunset_view boolean not null default false;

alter table property
    add column if not exists has_water_access boolean not null default false;

alter table property
    add column if not exists has_beach_access boolean not null default false;


-- ============================================================
-- COMMON SEARCHABLE AMENITIES
-- These are stable enough to deserve first-class columns.
-- ============================================================

alter table property
    add column if not exists has_pool boolean not null default false;

alter table property
    add column if not exists has_generator boolean not null default false;

alter table property
    add column if not exists has_solar boolean not null default false;

alter table property
    add column if not exists is_furnished boolean not null default false;

alter table property
    add column if not exists is_gated boolean not null default false;


-- ============================================================
-- EDITORIAL / FREE-FORM CONTENT
-- TEXT is intentionally used for narrative content rather than
-- turning editorial concepts into rigid relational structures.
-- ============================================================

alter table property
    add column if not exists hero_title text;

alter table property
    add column if not exists tagline text;

alter table property
    add column if not exists short_description text;

alter table property
    add column if not exists editorial_description text;

alter table property
    add column if not exists public_remarks text;

alter table property
    add column if not exists architecture_notes text;

alter table property
    add column if not exists amenities_notes text;

alter table property
    add column if not exists lifestyle_notes text;


-- ============================================================
-- LISTING AGENT / OFFICE
-- listing_user_id remains our internal CulebraLuxe owner.
-- These fields can represent external MLS/listing information.
-- ============================================================

alter table property
    add column if not exists listing_agent_name text;

alter table property
    add column if not exists listing_agent_email text;

alter table property
    add column if not exists listing_agent_phone text;

alter table property
    add column if not exists listing_office text;


-- ============================================================
-- SOURCE / FUTURE IDX / MLS
-- ============================================================

alter table property
    add column if not exists source_type text not null default 'manual';

alter table property
    add column if not exists source_provider text;

alter table property
    add column if not exists source_listing_key text;

alter table property
    add column if not exists source_modified_at timestamptz;

alter table property
    add column if not exists last_synced_at timestamptz;


-- ============================================================
-- SEO
-- ============================================================

alter table property
    add column if not exists seo_title text;

alter table property
    add column if not exists seo_description text;


-- ============================================================
-- PROPERTY CONSTRAINTS / INDEXES
-- ============================================================

create unique index if not exists idx_property_slug_unique
    on property(slug)
    where slug is not null;

create index if not exists idx_property_status
    on property(status);

create index if not exists idx_property_featured
    on property(featured);

create index if not exists idx_property_list_price
    on property(list_price);

create index if not exists idx_property_bedrooms
    on property(bedrooms);

create index if not exists idx_property_bathrooms
    on property(bathrooms);

create index if not exists idx_property_neighborhood
    on property(neighborhood);

create index if not exists idx_property_property_type
    on property(property_type);

create index if not exists idx_property_source_listing_key
    on property(source_listing_key)
    where source_listing_key is not null;


-- ============================================================
-- MEDIA
--
-- Existing images remain BYTEA-backed.
-- Video assets live in Mux, while Neon stores the Mux identity
-- and descriptive metadata.
-- ============================================================

alter table media
    add column if not exists media_type text not null default 'image';

alter table media
    alter column file_data drop not null;

alter table media
    add column if not exists mux_asset_id text;

alter table media
    add column if not exists mux_playback_id text;

alter table media
    add column if not exists duration_seconds numeric(12,3);

alter table media
    add column if not exists aspect_ratio text;

alter table media
    add column if not exists source_url text;


-- ============================================================
-- MEDIA CONSTRAINTS / INDEXES
-- ============================================================

alter table media
    drop constraint if exists media_media_type_check;

alter table media
    add constraint media_media_type_check
    check (media_type in ('image', 'video'));

create index if not exists idx_media_type
    on media(media_type);

create index if not exists idx_media_mux_playback_id
    on media(mux_playback_id)
    where mux_playback_id is not null;


-- ============================================================
-- PROPERTY / MEDIA ROLES
--
-- hero/gallery = photography
-- video        = long-form property film
-- short        = vertical/social short-form video
-- ============================================================

alter table property_media
    drop constraint if exists property_media_role_check;

alter table property_media
    add constraint property_media_role_check
    check (role in ('hero', 'gallery', 'video', 'short'));
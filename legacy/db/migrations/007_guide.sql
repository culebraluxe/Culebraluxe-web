-- 007_guide.sql
-- Island Guide schema: guide_item table and active-section sort index.
-- Guide seed content is owned by 008_seed_guide_items.sql
-- (idempotent upsert by unique slug).

CREATE TABLE guide_item (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),

    slug text NOT NULL UNIQUE,
    section text NOT NULL,
    name text NOT NULL,

    eyebrow text,
    subtitle text,
    area text,
    description text NOT NULL,
    note text,

    address text,
    phone text,
    website_url text,

    latitude numeric(9,6),
    longitude numeric(9,6),

    sort_order integer NOT NULL DEFAULT 0,
    is_active boolean NOT NULL DEFAULT true,

    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT guide_item_latitude_check
        CHECK (latitude IS NULL OR latitude BETWEEN -90 AND 90),

    CONSTRAINT guide_item_longitude_check
        CHECK (longitude IS NULL OR longitude BETWEEN -180 AND 180)
);

CREATE INDEX guide_item_section_sort_idx
    ON guide_item (section, sort_order)
    WHERE is_active = true;

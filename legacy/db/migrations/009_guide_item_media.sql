-- 009_guide_item_media.sql
-- Join table between guide_item and reusable media.
-- Keeps Guide media separate from property_media while reusing the canonical media table.

BEGIN;

CREATE TABLE guide_item_media (
    guide_item_id uuid NOT NULL
        REFERENCES guide_item(id)
        ON DELETE CASCADE,

    media_id uuid NOT NULL
        REFERENCES media(id)
        ON DELETE CASCADE,

    role text NOT NULL DEFAULT 'gallery',
    sort_order integer NOT NULL DEFAULT 0,

    created_at timestamptz NOT NULL DEFAULT now(),

    PRIMARY KEY (guide_item_id, media_id),

    CONSTRAINT guide_item_media_role_check
        CHECK (role IN ('card', 'gallery'))
);

CREATE INDEX guide_item_media_item_role_sort_idx
    ON guide_item_media (guide_item_id, role, sort_order);

COMMIT;

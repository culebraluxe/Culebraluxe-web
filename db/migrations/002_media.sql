CREATE TABLE media (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    file_data BYTEA NOT NULL,

    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    file_size BIGINT,

    width INTEGER,
    height INTEGER,

    alt_text TEXT,
    caption TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


CREATE TABLE property_media (
    property_id UUID NOT NULL
        REFERENCES property(id)
        ON DELETE CASCADE,

    media_id UUID NOT NULL
        REFERENCES media(id)
        ON DELETE CASCADE,

    role TEXT NOT NULL DEFAULT 'gallery'
        CHECK (role IN ('hero', 'gallery')),

    sort_order INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (property_id, media_id)
);


CREATE INDEX idx_property_media_property
    ON property_media(property_id);

CREATE INDEX idx_property_media_media
    ON property_media(media_id);

CREATE INDEX idx_property_media_role
    ON property_media(property_id, role);
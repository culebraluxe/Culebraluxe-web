-- CulebraLuxe Public Site
-- PX-25 — Managed Marketing Content (schema)
-- Migration: 063_marketing_content.sql
--
-- Establishes the Neon-backed managed-content surface for public-site
-- editorial copy. Marketing copy is content, not code: it is stored in
-- explicit relational fields (mirroring the guide_item precedent) and read
-- server-side at request time, so a copy edit no longer requires a code
-- deploy. Sanity is retired (054); Neon remains the canonical content store.
--
-- Model:
--   marketing_content        — one row per content SLOT (stable text id, e.g.
--                              'home.hero'). Slots are the identity; a slot
--                              renders at exactly one place on the site.
--   marketing_content_item   — ordered child rows for structured copy inside a
--                              slot: list items, stat pairs, paragraphs, FAQ
--                              question/answer pairs, contact facts.
--                              (item_key discriminates the role.)
--
-- Seed content is owned by 064_seed_marketing_content.sql (idempotent upsert
-- by slot id, mirroring the guide seed split 007/008).
--
-- Applied to the disposable DEV branch as part of PX-25. Promotion to
-- production happens only through the explicit production-release task.

begin;

create table if not exists marketing_content (
    id text primary key,

    -- Slot role. The site renders each kind at a known place; kinds are the
    -- bounded vocabulary of editorial surfaces this story manages.
    kind text not null
        check (kind in ('hero', 'services', 'culture', 'about', 'contact', 'faq')),

    title text,
    subtitle text,
    eyebrow text,
    body text,

    cta_label text,
    cta_href text,

    image_path text,
    image_alt text,

    sort_order integer not null default 0,
    is_active boolean not null default true,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists marketing_content_item (
    id uuid primary key default gen_random_uuid(),

    content_id text not null
        references marketing_content (id)
        on delete cascade,

    -- Role of this child row within its slot:
    --   'list'      — checklist line (services)
    --   'stat'      — stat pair (label = key, value = description)
    --   'paragraph' — extra editorial paragraph
    --   'office'    — contact fact: label = 'Office', value = address
    --   'email'     — contact fact: label = 'Enquiries', value = address
    --   'faq'       — question/answer pair (label = question, value = answer)
    item_key text not null,

    label text,
    value text,

    sort_order integer not null default 0,
    is_active boolean not null default true,

    created_at timestamptz not null default now()
);

create index if not exists marketing_content_kind_sort_idx
    on marketing_content (kind, sort_order, id)
    where is_active = true;

create index if not exists marketing_content_item_sort_idx
    on marketing_content_item (content_id, sort_order, created_at)
    where is_active = true;

commit;

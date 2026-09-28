-- CulebraLuxe Public Site
-- PX-25 — Managed Marketing Content (seed)
-- Migration: 064_seed_marketing_content.sql
--
-- Idempotent upsert (by slot id) of the current homepage + FAQ editorial copy
-- into marketing_content / marketing_content_item. Every string is
-- byte-identical to the JSX it replaces (components/hero.tsx, services.tsx,
-- culture.tsx, about.tsx, contact.tsx; app/faq/page.tsx) so rendered output
-- is preserved exactly while the source of truth moves into managed content.
--
-- Safe to rerun: rows are upserted by stable slot id; item child rows are
-- deleted and re-inserted for the slot so the seed is the authoritative copy.

begin;

insert into marketing_content
    (id, kind, title, subtitle, eyebrow, body, cta_label, cta_href, image_path, image_alt, sort_order)
values
  ('home.hero', 'hero', 'An island held quietly between sea and sky.', null,
   'Culebra · Puerto Rico',
   'A curated portfolio of architectural residences and beachfront estates, presented with the discretion the island deserves.',
   'View the Collection', '#properties', '/images/hero-villa.png',
   'Cliffside modern villa overlooking the turquoise Caribbean sea in Culebra', 10),

  ('home.services.buyers', 'services', 'A considered path to your place on the island.', null,
   'For Buyers',
   'We represent a small number of buyers each year, guiding every stage with discretion — from private viewings and title diligence to residency, architecture, and the quiet logistics of island life.',
   'Begin a search', '#contact', null, null, 10),

  ('home.services.sellers', 'services', 'Presented to the few who truly belong here.', null,
   'For Sellers',
   'Your home deserves more than a listing. We craft an editorial presentation — considered photography, measured storytelling, and introductions to a private network of international buyers who understand Culebra.',
   'Request a valuation', '#contact', '/images/coastline.png',
   'Aerial view of the Culebra coastline with jade and turquoise water', 20),

  ('home.culture', 'culture', 'A slower rhythm, kept intentionally intact.', 'Life on Culebra',
   'Island Culture',
   'No traffic lights. No high-rises. Fishing boats at dawn, reef-clear water by noon, and evenings measured in shades of gold. Culebra rewards those who choose to arrive quietly and stay attentively.',
   null, null, '/images/culture.png',
   'The white sand crescent and clear turquoise water of Flamenco Beach, Culebra', 10),

  ('home.about', 'about',
   'CulebraLuxe is a boutique brokerage devoted to a single island. We work with few clients, few homes, and an uncommon amount of care.',
   null, 'About Us',
   'Founded by island residents, we know Culebra beyond its coordinates — the trade winds, the tide charts, the families who have shaped it for generations.',
   null, null, null, null, 10),

  ('home.contact', 'contact', 'Let''s begin a quiet conversation.', null, 'Contact',
   null, null, null, null, null, 10),

  ('contact.page-hero', 'contact', 'Let''s begin a quiet conversation.', null, 'Contact',
   'Whether you are considering a purchase, a sale, or simply the possibility of island life, we would be glad to hear from you.',
   null, null, '/images/coastline.png',
   'The Culebra coastline at golden hour', 20),

  ('faq.list', 'faq', null, 'Have a question we have not answered here?', null,
   null, 'Ask us directly', '/contact', null, null, 10),

  ('faq.page-hero', 'faq', 'Questions, quietly answered.', null, 'Frequently Asked',
   'A few of the things buyers and sellers most often ask us about life and property on Culebra.',
   null, null, '/images/hero-villa.png',
   'A luxury villa overlooking the Culebra coastline', 20)
on conflict (id) do update set
    kind = excluded.kind,
    title = excluded.title,
    subtitle = excluded.subtitle,
    eyebrow = excluded.eyebrow,
    body = excluded.body,
    cta_label = excluded.cta_label,
    cta_href = excluded.cta_href,
    image_path = excluded.image_path,
    image_alt = excluded.image_alt,
    sort_order = excluded.sort_order,
    is_active = excluded.is_active,
    updated_at = now();

-- Child rows: seed is the authoritative copy per slot (delete + insert).
delete from marketing_content_item
where content_id in (
    'home.hero', 'home.services.buyers', 'home.services.sellers',
    'home.culture', 'home.about', 'home.contact', 'contact.page-hero',
    'faq.list', 'faq.page-hero'
);

insert into marketing_content_item (content_id, item_key, label, value, sort_order)
values
  -- home.services.buyers — checklist
  ('home.services.buyers', 'list', null, 'Private, unlisted viewings', 10),
  ('home.services.buyers', 'list', null, 'Legal, title & closing guidance', 20),
  ('home.services.buyers', 'list', null, 'Architecture & renovation introductions', 30),

  -- home.culture — island stats
  ('home.culture', 'stat', 'Flamenco', 'Consistently ranked among the world’s finest beaches.', 10),
  ('home.culture', 'stat', 'Marine Reserve', 'Protected reefs and cays surround the island.', 20),
  ('home.culture', 'stat', '30 Minutes', 'A short flight or ferry from mainland Puerto Rico.', 30),

  -- home.about — second editorial paragraph + stats
  ('home.about', 'paragraph', null,
   'We measure success not in volume but in fit — pairing the right stewards with the right homes, and protecting the character that makes this place rare.', 10),
  ('home.about', 'stat', '14', 'Years on island', 10),
  ('home.about', 'stat', '1', 'Island, entirely', 20),

  -- home.contact — contact facts
  ('home.contact', 'office', 'Office', 'Calle Escudero, Dewey, Culebra, PR 00775', 10),
  ('home.contact', 'email', 'Enquiries', 'hello@culebraluxe.com', 20),

  -- faq.list — question/answer pairs
  ('faq.list', 'faq', 'Can anyone buy property in Culebra?',
   'Yes. Culebra is part of Puerto Rico, a United States territory, so U.S. citizens buy here without restriction and the process will feel familiar. Buyers from elsewhere are welcome as well; we guide every client through the particulars.', 10),
  ('faq.list', 'faq', 'How do I actually get to the island?',
   'Culebra sits about 30 minutes from mainland Puerto Rico by a short flight from San Juan or Ceiba, or by ferry from Ceiba. Once here, most residents move about by golf cart — there are no traffic lights on the island.', 20),
  ('faq.list', 'faq', 'Are many of your homes off-market?',
   'Often, yes. Many of Culebra’s finest residences never reach a public listing. We arrange discreet viewings of off-market homes held within our private network, matched to what each buyer is genuinely seeking.', 30),
  ('faq.list', 'faq', 'What are the ongoing costs of owning here?',
   'Beyond the purchase, owners should plan for property taxes, insurance, utilities, and — for many homes — management or caretaking while away. We provide clear estimates for any specific property and introductions to trusted local services.', 40),
  ('faq.list', 'faq', 'Can you help with residency or relocation?',
   'We support clients through the practical logistics of island life, from residency questions to introductions for architects, builders, and property managers. Our involvement does not end at closing.', 50),
  ('faq.list', 'faq', 'How do you work with sellers?',
   'We take on a small number of listings and give each an editorial presentation — considered photography, measured storytelling, and introductions to a private circle of international buyers. It begins with a confidential valuation.', 60);

commit;

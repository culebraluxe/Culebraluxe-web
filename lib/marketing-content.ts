// ---------------------------------------------------------------------------
// PX-25 — Managed Marketing Content: canonical public-site content contract.
//
// The public site's editorial copy lives in Neon (marketing_content /
// marketing_content_item, migrations 063/064) and is read server-side at
// request time through db/marketing-content.ts. This module is the ONE typed
// contract for that surface: slot identity, block/item shapes, and the pure
// selectors pages and components use to turn a flat block list into the
// shapes each surface renders. No JSX literals, no SQL, no React.
//
// Every selector here is pure so the contract is unit-testable without Neon
// (see workflow_app/tests/marketing-content.test.ts).
// ---------------------------------------------------------------------------

/** Bounded vocabulary of editorial surfaces this story manages. */
export const MARKETING_CONTENT_KINDS = [
  'hero',
  'services',
  'culture',
  'about',
  'contact',
  'faq',
] as const

export type MarketingContentKind = (typeof MARKETING_CONTENT_KINDS)[number]

/** Stable slot identity — a slot renders at exactly one place on the site. */
export const MARKETING_SLOTS = {
  hero: 'home.hero',
  buyers: 'home.services.buyers',
  sellers: 'home.services.sellers',
  culture: 'home.culture',
  about: 'home.about',
  contact: 'home.contact',
  contactPageHero: 'contact.page-hero',
  faqList: 'faq.list',
  faqPageHero: 'faq.page-hero',
} as const

export type MarketingContentItem = {
  /** Role within its slot: 'list' | 'stat' | 'paragraph' | 'office' | 'email' | 'faq'. */
  key: string
  /** Stat key / FAQ question / contact label; null for plain list lines. */
  label: string | null
  /** List line / stat value / FAQ answer / contact value. */
  value: string | null
}

export type MarketingContentBlock = {
  id: string
  kind: MarketingContentKind
  title: string | null
  subtitle: string | null
  eyebrow: string | null
  body: string | null
  ctaLabel: string | null
  ctaHref: string | null
  imagePath: string | null
  imageAlt: string | null
  items: MarketingContentItem[]
}

/** The homepage's six editorial slots (migration 064 seed ids). */
export type HomeContent = {
  hero: MarketingContentBlock | null
  buyers: MarketingContentBlock | null
  sellers: MarketingContentBlock | null
  culture: MarketingContentBlock | null
  about: MarketingContentBlock | null
  contact: MarketingContentBlock | null
}

/** One question/answer pair rendered by the FAQ accordion. */
export type FaqEntry = { q: string; a: string }

/** The /contact page's managed surface: hero block + contact block. */
export type ContactPageContent = {
  hero: MarketingContentBlock | null
  contact: MarketingContentBlock | null
}

/** The FAQ page's managed surface: hero block + accordion entries + CTA. */
export type FaqPageContent = {
  hero: MarketingContentBlock | null
  entries: FaqEntry[]
  ctaHeading: string | null
  ctaLabel: string | null
  ctaHref: string | null
}

export function blockById(
  blocks: readonly MarketingContentBlock[],
  id: string,
): MarketingContentBlock | null {
  return blocks.find((block) => block.id === id) ?? null
}

/**
 * Child rows of `block` whose role is `key`, in stored order. Rows are
 * already active-filtered by the repository; this selector only narrows by
 * role and preserves the deterministic ordering.
 */
export function itemsFor(
  block: MarketingContentBlock,
  key: string,
): MarketingContentItem[] {
  return block.items.filter((item) => item.key === key)
}

/** Homepage surface assembled from the flat repository list (never throws). */
export function buildHomeContent(
  blocks: readonly MarketingContentBlock[],
): HomeContent {
  return {
    hero: blockById(blocks, MARKETING_SLOTS.hero),
    buyers: blockById(blocks, MARKETING_SLOTS.buyers),
    sellers: blockById(blocks, MARKETING_SLOTS.sellers),
    culture: blockById(blocks, MARKETING_SLOTS.culture),
    about: blockById(blocks, MARKETING_SLOTS.about),
    contact: blockById(blocks, MARKETING_SLOTS.contact),
  }
}

export function faqEntries(block: MarketingContentBlock): FaqEntry[] {
  return itemsFor(block, 'faq')
    .filter(
      (item): item is MarketingContentItem & { label: string } =>
        item.label !== null && item.value !== null,
    )
    .map((item) => ({ q: item.label, a: item.value as string }))
}

/** /contact page surface assembled from the flat repository list. */
export function buildContactPageContent(
  blocks: readonly MarketingContentBlock[],
): ContactPageContent {
  return {
    hero: blockById(blocks, MARKETING_SLOTS.contactPageHero),
    contact: blockById(blocks, MARKETING_SLOTS.contact),
  }
}

/** FAQ page surface assembled from the flat repository list. */
export function buildFaqPageContent(
  blocks: readonly MarketingContentBlock[],
): FaqPageContent {
  const list = blockById(blocks, MARKETING_SLOTS.faqList)
  return {
    hero: blockById(blocks, MARKETING_SLOTS.faqPageHero),
    entries: list ? faqEntries(list) : [],
    ctaHeading: list?.subtitle ?? null,
    ctaLabel: list?.ctaLabel ?? null,
    ctaHref: list?.ctaHref ?? null,
  }
}

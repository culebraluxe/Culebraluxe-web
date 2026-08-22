import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  blockById,
  buildContactPageContent,
  buildFaqPageContent,
  buildHomeContent,
  faqEntries,
  itemsFor,
  MARKETING_SLOTS,
  type MarketingContentBlock,
} from '../../lib/marketing-content'
import { getMarketingContent } from '../../db/marketing-content'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// PX-25 — Managed Marketing Content: targeted contract + repository tests.
// Zero Neon, zero React: the canonical selectors (lib/marketing-content.ts)
// are pure, and the repository (db/marketing-content.ts) is exercised through
// an in-memory fake QueryExecutor (the db/storyboard.ts test convention).
// ---------------------------------------------------------------------------

type Row = Record<string, any>

function block(overrides: Partial<MarketingContentBlock>): MarketingContentBlock {
  return {
    id: 'x',
    kind: 'hero',
    title: null,
    subtitle: null,
    eyebrow: null,
    body: null,
    ctaLabel: null,
    ctaHref: null,
    imagePath: null,
    imageAlt: null,
    items: [],
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// Canonical contract (pure selectors)
// ---------------------------------------------------------------------------

test('blockById finds a block by stable slot id and misses unknown ids', () => {
  const blocks = [block({ id: 'home.hero' }), block({ id: 'faq.list' })]
  assert.equal(blockById(blocks, 'home.hero')?.id, 'home.hero')
  assert.equal(blockById(blocks, 'faq.list')?.id, 'faq.list')
  assert.equal(blockById(blocks, 'nope'), null)
})

test('itemsFor narrows by role and preserves stored order', () => {
  const b = block({
    items: [
      { key: 'stat', label: 'A', value: '1' },
      { key: 'list', label: null, value: 'line' },
      { key: 'stat', label: 'B', value: '2' },
    ],
  })
  assert.deepEqual(itemsFor(b, 'stat'), [
    { key: 'stat', label: 'A', value: '1' },
    { key: 'stat', label: 'B', value: '2' },
  ])
  assert.deepEqual(itemsFor(b, 'list'), [{ key: 'list', label: null, value: 'line' }])
  assert.deepEqual(itemsFor(b, 'office'), [])
})

test('buildHomeContent maps every homepage slot; missing slots stay null', () => {
  const blocks = [
    block({ id: MARKETING_SLOTS.hero, title: 'Hero' }),
    block({ id: MARKETING_SLOTS.buyers }),
    block({ id: MARKETING_SLOTS.sellers }),
    block({ id: MARKETING_SLOTS.culture }),
    block({ id: MARKETING_SLOTS.about }),
    block({ id: MARKETING_SLOTS.contact }),
  ]
  const home = buildHomeContent(blocks)
  assert.equal(home.hero?.title, 'Hero')
  assert.equal(home.buyers?.id, MARKETING_SLOTS.buyers)
  assert.equal(home.sellers?.id, MARKETING_SLOTS.sellers)
  assert.equal(home.culture?.id, MARKETING_SLOTS.culture)
  assert.equal(home.about?.id, MARKETING_SLOTS.about)
  assert.equal(home.contact?.id, MARKETING_SLOTS.contact)

  const partial = buildHomeContent([block({ id: MARKETING_SLOTS.hero })])
  assert.equal(partial.hero?.id, MARKETING_SLOTS.hero)
  assert.equal(partial.contact, null)
  assert.equal(partial.sellers, null)
})

test('buildHomeContent tolerates an empty repository list', () => {
  const home = buildHomeContent([])
  assert.deepEqual(home, {
    hero: null,
    buyers: null,
    sellers: null,
    culture: null,
    about: null,
    contact: null,
  })
})

test('faqEntries builds q/a pairs and drops incomplete rows', () => {
  const b = block({
    items: [
      { key: 'faq', label: 'Q1', value: 'A1' },
      { key: 'faq', label: 'Q2', value: 'A2' },
      { key: 'faq', label: null, value: 'orphan' },
      { key: 'faq', label: 'no-answer', value: null },
      { key: 'other', label: 'not-faq', value: 'ignored' },
    ],
  })
  assert.deepEqual(faqEntries(b), [
    { q: 'Q1', a: 'A1' },
    { q: 'Q2', a: 'A2' },
  ])
})

test('buildFaqPageContent assembles hero, entries and CTA from blocks', () => {
  const blocks = [
    block({
      id: MARKETING_SLOTS.faqList,
      subtitle: 'Have a question?',
      ctaLabel: 'Ask us directly',
      ctaHref: '/contact',
      items: [
        { key: 'faq', label: 'Q', value: 'A' },
        { key: 'faq', label: 'Q2', value: 'A2' },
      ],
    }),
    block({ id: MARKETING_SLOTS.faqPageHero, title: 'Questions' }),
  ]
  const page = buildFaqPageContent(blocks)
  assert.equal(page.hero?.title, 'Questions')
  assert.deepEqual(page.entries, [
    { q: 'Q', a: 'A' },
    { q: 'Q2', a: 'A2' },
  ])
  assert.equal(page.ctaHeading, 'Have a question?')
  assert.equal(page.ctaLabel, 'Ask us directly')
  assert.equal(page.ctaHref, '/contact')
})

test('buildFaqPageContent tolerates a missing FAQ list block', () => {
  const page = buildFaqPageContent([block({ id: MARKETING_SLOTS.faqPageHero })])
  assert.deepEqual(page.entries, [])
  assert.equal(page.ctaHeading, null)
  assert.equal(page.hero?.id, MARKETING_SLOTS.faqPageHero)
})

test('buildContactPageContent assembles hero and contact blocks', () => {
  const page = buildContactPageContent([
    block({ id: MARKETING_SLOTS.contactPageHero, title: 'Contact' }),
    block({ id: MARKETING_SLOTS.contact, title: "Let's begin" }),
  ])
  assert.equal(page.hero?.title, 'Contact')
  assert.equal(page.contact?.title, "Let's begin")
  assert.equal(buildContactPageContent([]).hero, null)
})

// ---------------------------------------------------------------------------
// Repository (fake QueryExecutor)
// ---------------------------------------------------------------------------

class FakeMarketingDb {
  blockRows: Row[] = []
  itemRows: Row[] = []

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ..._params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < _params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    // The items SELECT is the only one that mentions the child table.
    if (t.includes('from marketing_content_item')) {
      return Promise.resolve(this.itemRows)
    }
    if (t.includes('from marketing_content')) {
      return Promise.resolve(this.blockRows)
    }
    return Promise.resolve([])
  }
}

test('getMarketingContent returns blocks with items attached, in row order', async () => {
  const fake = new FakeMarketingDb()
  fake.blockRows = [
    {
      id: 'home.hero',
      kind: 'hero',
      title: 'An island held quietly between sea and sky.',
      subtitle: null,
      eyebrow: 'Culebra · Puerto Rico',
      body: 'A curated portfolio…',
      cta_label: 'View the Collection',
      cta_href: '#properties',
      image_path: '/images/hero-villa.png',
      image_alt: 'Cliffside villa',
      sort_order: 10,
    },
    {
      id: 'home.services.buyers',
      kind: 'services',
      title: 'Buyers',
      subtitle: null,
      eyebrow: 'For Buyers',
      body: null,
      cta_label: 'Begin a search',
      cta_href: '#contact',
      image_path: null,
      image_alt: null,
      sort_order: 10,
    },
  ]
  fake.itemRows = [
    { content_id: 'home.hero', item_key: 'stat', label: 'A', value: '1', sort_order: 20 },
    { content_id: 'home.hero', item_key: 'stat', label: 'B', value: '2', sort_order: 10 },
    { content_id: 'home.services.buyers', item_key: 'list', label: null, value: 'line', sort_order: 10 },
    { content_id: 'unknown-slot', item_key: 'list', label: null, value: 'orphan', sort_order: 10 },
  ]

  const blocks = await getMarketingContent(fake.tx)
  assert.equal(blocks.length, 2)

  const hero = blocks[0]
  assert.equal(hero.id, 'home.hero')
  assert.equal(hero.kind, 'hero')
  assert.equal(hero.title, 'An island held quietly between sea and sky.')
  assert.equal(hero.eyebrow, 'Culebra · Puerto Rico')
  assert.equal(hero.ctaLabel, 'View the Collection')
  assert.equal(hero.ctaHref, '#properties')
  assert.equal(hero.imagePath, '/images/hero-villa.png')
  assert.equal(hero.imageAlt, 'Cliffside villa')
  assert.equal(hero.body, 'A curated portfolio…')
  // Items keep the stored order (not re-sorted by the mapper).
  assert.deepEqual(hero.items, [
    { key: 'stat', label: 'A', value: '1' },
    { key: 'stat', label: 'B', value: '2' },
  ])

  const buyers = blocks[1]
  assert.equal(buyers.id, 'home.services.buyers')
  assert.deepEqual(buyers.items, [{ key: 'list', label: null, value: 'line' }])

  // Item rows referencing a missing slot never leak into any block.
  assert.ok(blocks.every((b) => !b.items.some((i) => i.value === 'orphan')))
})

test('getMarketingContent maps null columns to null and empty lists to []', async () => {
  const fake = new FakeMarketingDb()
  fake.blockRows = [
    {
      id: 'faq.list',
      kind: 'faq',
      title: null,
      subtitle: null,
      eyebrow: null,
      body: null,
      cta_label: null,
      cta_href: null,
      image_path: null,
      image_alt: null,
      sort_order: 10,
    },
  ]
  fake.itemRows = []

  const blocks = await getMarketingContent(fake.tx)
  assert.equal(blocks.length, 1)
  assert.deepEqual(blocks[0], {
    id: 'faq.list',
    kind: 'faq',
    title: null,
    subtitle: null,
    eyebrow: null,
    body: null,
    ctaLabel: null,
    ctaHref: null,
    imagePath: null,
    imageAlt: null,
    items: [],
  })
})

test('getMarketingContent returns [] when the tables hold no rows', async () => {
  const fake = new FakeMarketingDb()
  fake.blockRows = []
  fake.itemRows = []
  assert.deepEqual(await getMarketingContent(fake.tx), [])
})

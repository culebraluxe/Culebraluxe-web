import type {
  MarketingContentBlock,
  MarketingContentItem,
  MarketingContentKind,
} from '../lib/marketing-content'
import { db } from './database-gateway'
import type { Result } from './database-gateway'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// PX-25 — marketing content repository (migrations 063/064).
//
// Reads the managed public-site editorial copy at request time. The SELECTs
// are the entire surface: blocks (one row per slot) plus ordered child items,
// joined in memory so the canonical contract (lib/marketing-content.ts) stays
// pure. Follows the db/storyboard.ts convention of a lazily resolved default
// executor so importing this module never requires a DATABASE_URL; tests
// inject an in-memory fake.
// ---------------------------------------------------------------------------

type MarketingContentBlockRow = QueryRow & {
  id: string
  kind: string
  title: string | null
  subtitle: string | null
  eyebrow: string | null
  body: string | null
  cta_label: string | null
  cta_href: string | null
  image_path: string | null
  image_alt: string | null
  sort_order: number
}

type MarketingContentItemRow = QueryRow & {
  content_id: string
  item_key: string
  label: string | null
  value: string | null
  sort_order: number
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function mapItem(row: MarketingContentItemRow): MarketingContentItem {
  return {
    key: row.item_key,
    label: row.label ?? null,
    value: row.value ?? null,
  }
}

function mapBlock(
  row: MarketingContentBlockRow,
  items: MarketingContentItem[],
): MarketingContentBlock {
  return {
    id: row.id,
    kind: row.kind as MarketingContentKind,
    title: row.title ?? null,
    subtitle: row.subtitle ?? null,
    eyebrow: row.eyebrow ?? null,
    body: row.body ?? null,
    ctaLabel: row.cta_label ?? null,
    ctaHref: row.cta_href ?? null,
    imagePath: row.image_path ?? null,
    imageAlt: row.image_alt ?? null,
    items,
  }
}

export async function getMarketingContent(): Promise<Result<MarketingContentBlock[]>> {
  // DB-HARDEN-01C — public read: never throw a DB failure; return a Result.
  const blockR = await db.query<MarketingContentBlockRow>`
    select id, kind, title, subtitle, eyebrow, body, cta_label, cta_href,
      image_path, image_alt, sort_order
    from marketing_content
    where is_active = true
    order by kind, sort_order, id
  `
  if (!blockR.ok) return blockR
  const blockRows = blockR.data

  const itemR = await db.query<MarketingContentItemRow>`
    select content_id, item_key, label, value, sort_order
    from marketing_content_item
    where is_active = true
    order by content_id, sort_order, created_at
  `
  if (!itemR.ok) return itemR
  const itemRows = itemR.data

  const itemsByContentId = new Map<string, MarketingContentItem[]>()
  for (const row of itemRows) {
    const bucket = itemsByContentId.get(row.content_id) ?? []
    bucket.push(mapItem(row))
    itemsByContentId.set(row.content_id, bucket)
  }

  return {
    ok: true,
    data: blockRows.map((row) => mapBlock(row, itemsByContentId.get(row.id) ?? [])),
  }
}

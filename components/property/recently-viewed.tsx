'use client'

import Link from 'next/link'
import { useEffect, useState } from 'react'

// Distinct from the Favorites key (culebraluxe:saved-properties) so the two
// browser-local features never collide. Stores only public, non-sensitive
// listing facts (slug, canonical id, public name, timestamp).
const STORAGE_KEY = 'culebraluxe:recently-viewed'
const MAX = 6

type RecentEntry = {
  slug: string
  id: string
  name: string
  at: number
}

function readRecent(): RecentEntry[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (entry): entry is RecentEntry =>
        Boolean(entry) &&
        typeof (entry as RecentEntry).slug === 'string' &&
        typeof (entry as RecentEntry).id === 'string' &&
        typeof (entry as RecentEntry).name === 'string' &&
        typeof (entry as RecentEntry).at === 'number',
    )
  } catch {
    return []
  }
}

type RecentlyViewedProps = {
  slug: string
  id: string
  name: string
  validSlugs: string[]
}

export function RecentlyViewed({
  slug,
  id,
  name,
  validSlugs,
}: RecentlyViewedProps) {
  const [mounted, setMounted] = useState(false)
  const [items, setItems] = useState<RecentEntry[]>([])

  useEffect(() => {
    setMounted(true)

    // Record the current visit: dedupe on canonical id (and slug), newest
    // first, bounded to MAX entries.
    const entry: RecentEntry = { slug, id, name, at: Date.now() }
    const recorded = [
      entry,
      ...readRecent().filter((existing) => existing.id !== id),
    ].slice(0, MAX)

    // Drop stale/non-public entries from storage (always keep the current
    // entry) so delisted listings cannot permanently consume a history slot.
    const valid = new Set(validSlugs)
    const pruned = recorded.filter(
      (entry) => entry.slug === slug || valid.has(entry.slug),
    )

    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(pruned))
    } catch {
      // ignore storage failures (private mode, etc.)
    }

    // Render only entries that still correspond to live public listings,
    // excluding the current property.
    const visible = pruned
      .filter((entry) => entry.slug !== slug && valid.has(entry.slug))
      .slice(0, MAX)

    setItems(visible)
  }, [slug, id, name, validSlugs])

  if (!mounted || items.length === 0) {
    return null
  }

  return (
    <nav
      aria-label="Recently viewed properties"
      className="mt-16 border-t border-border pt-8 md:mt-20"
    >
      <p className="text-xs font-light uppercase tracking-[0.34em] text-accent">
        Recently Viewed
      </p>
      <ul className="mt-6 flex flex-wrap gap-x-10 gap-y-4">
        {items.map((item) => (
          <li key={item.id}>
            <Link
              href={`/properties/${item.slug}`}
              className="group inline-flex min-h-11 items-center gap-3 font-serif text-lg font-light text-foreground transition-colors duration-300 hover:text-accent"
            >
              <span>{item.name}</span>
              <span
                aria-hidden
                className="inline-block h-px w-5 bg-accent/60 transition-all duration-500 group-hover:w-8"
              />
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  )
}

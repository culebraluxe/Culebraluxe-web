'use client'

import { useEffect, useState } from 'react'
import { Heart } from 'lucide-react'
import { cn } from '@/lib/utils'

const STORAGE_KEY = 'culebraluxe:saved-properties'

export type SavedPropertyEntry = {
  id: string
  slug: string
  name: string
}

// Backward compatible: reads both the legacy array-of-id format and the
// current array-of-{id,slug,name} format. Canonical ids only; no sensitive data.
export function readSaved(): SavedPropertyEntry[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed
      .map((entry): SavedPropertyEntry | null => {
        if (typeof entry === 'string') {
          return entry ? { id: entry, slug: '', name: '' } : null
        }
        if (entry && typeof (entry as SavedPropertyEntry).id === 'string') {
          return {
            id: (entry as SavedPropertyEntry).id,
            slug:
              typeof (entry as SavedPropertyEntry).slug === 'string'
                ? (entry as SavedPropertyEntry).slug
                : '',
            name:
              typeof (entry as SavedPropertyEntry).name === 'string'
                ? (entry as SavedPropertyEntry).name
                : '',
          }
        }
        return null
      })
      .filter((entry): entry is SavedPropertyEntry => Boolean(entry))
  } catch {
    return []
  }
}

function writeSaved(entries: SavedPropertyEntry[]) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(entries))
  } catch {
    // ignore storage failures (private mode, etc.)
  }
}

type SavePropertyProps = {
  propertyId: string
  slug?: string
  name?: string
  className?: string
  variant?: 'button' | 'icon'
}

export function SaveProperty({
  propertyId,
  slug = '',
  name = '',
  className,
  variant = 'button',
}: SavePropertyProps) {
  const [saved, setSaved] = useState(false)
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
    setSaved(readSaved().some((entry) => entry.id === propertyId))
  }, [propertyId])

  const toggle = () => {
    const current = readSaved()
    const exists = current.some((entry) => entry.id === propertyId)
    const next = exists
      ? current.filter((entry) => entry.id !== propertyId)
      : [...current, { id: propertyId, slug, name }]
    writeSaved(next)
    setSaved(!exists)
  }

  const label = saved ? 'Saved' : 'Save Property'

  if (variant === 'icon') {
    return (
      <button
        type="button"
        onClick={toggle}
        aria-pressed={mounted ? saved : undefined}
        aria-label={label}
        className={cn(
          'flex h-9 w-9 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background',
          className,
        )}
      >
        <Heart
          className={cn(
            'h-4 w-4 transition-all duration-300',
            mounted && saved ? 'fill-accent text-accent' : 'text-foreground',
          )}
        />
      </button>
    )
  }

  return (
    <button
      type="button"
      onClick={toggle}
      aria-pressed={mounted ? saved : undefined}
      className={cn(
        'group inline-flex items-center justify-center gap-2.5 border border-border px-8 py-4 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors duration-500 hover:border-foreground',
        className,
      )}
    >
      <Heart
        className={cn(
          'h-4 w-4 transition-all duration-300',
          mounted && saved ? 'fill-accent text-accent' : 'text-foreground',
        )}
      />
      {label}
    </button>
  )
}

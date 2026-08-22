'use client'

import { useEffect, useState } from 'react'
import { Heart } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  FAVORITES_CHANGED_EVENT,
  isFavorite,
  toggleFavorite,
} from '@/lib/favorites'

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

    const sync = () => setSaved(isFavorite(propertyId))

    sync()
    window.addEventListener(FAVORITES_CHANGED_EVENT, sync)
    return () => window.removeEventListener(FAVORITES_CHANGED_EVENT, sync)
  }, [propertyId])

  const toggle = () => {
    setSaved(toggleFavorite({ id: propertyId, slug, name }))
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

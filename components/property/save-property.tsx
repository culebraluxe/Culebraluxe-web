'use client'

import { useEffect, useState } from 'react'
import { Heart } from 'lucide-react'
import { cn } from '@/lib/utils'

const STORAGE_KEY = 'culebraluxe:saved-properties'

function readSaved(): string[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    return raw ? (JSON.parse(raw) as string[]) : []
  } catch {
    return []
  }
}

type SavePropertyProps = {
  propertyId: string
  className?: string
  variant?: 'button' | 'icon'
}

export function SaveProperty({
  propertyId,
  className,
  variant = 'button',
}: SavePropertyProps) {
  const [saved, setSaved] = useState(false)
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
    setSaved(readSaved().includes(propertyId))
  }, [propertyId])

  const toggle = () => {
    const current = readSaved()
    const next = current.includes(propertyId)
      ? current.filter((id) => id !== propertyId)
      : [...current, propertyId]
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
    } catch {
      // ignore storage failures (private mode, etc.)
    }
    setSaved(next.includes(propertyId))
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

'use client'

import { useEffect, useState } from 'react'
import { Heart } from 'lucide-react'

import {
  FAVORITES_CHANGED_EVENT,
  readFavorites,
} from '@/lib/favorites'

type FavoritesLinkProps = {
  className?: string
  /** Optional handler fired on click (e.g. closing the mobile menu). */
  onNavigate?: () => void
}

/**
 * Header link to /favorites with a live saved-count badge. Subscribes to the
 * favorites change event so the count stays in agreement with the hearts and
 * the saved-properties page without shared state.
 */
export function FavoritesLink({ className, onNavigate }: FavoritesLinkProps) {
  const [count, setCount] = useState(0)

  useEffect(() => {
    const sync = () => setCount(readFavorites().length)
    sync()
    window.addEventListener(FAVORITES_CHANGED_EVENT, sync)
    return () => window.removeEventListener(FAVORITES_CHANGED_EVENT, sync)
  }, [])

  const label = count > 0 ? `Saved properties (${count})` : 'Saved properties'

  return (
    <a
      href="/favorites"
      onClick={onNavigate}
      aria-label={label}
      className={className}
    >
      <Heart
        className="h-3.5 w-3.5"
        aria-hidden
        strokeWidth={1.5}
      />
      Saved
      {count > 0 && (
        <span
          aria-hidden
          className="text-[#c6a15b]"
        >
          {count}
        </span>
      )}
    </a>
  )
}

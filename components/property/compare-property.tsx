'use client'

import { useEffect, useState } from 'react'
import { Scale } from 'lucide-react'

import { cn } from '@/lib/utils'
import {
  COMPARE_CHANGED_EVENT,
  readCompare,
  toggleCompare,
} from '@/lib/compare'

type ComparePropertyProps = {
  id: string
  slug: string
  name: string
  className?: string
}

export function CompareProperty({
  id,
  slug,
  name,
  className,
}: ComparePropertyProps) {
  const [selected, setSelected] = useState(false)
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)

    const sync = () =>
      setSelected(readCompare().some((entry) => entry.id === id))

    sync()
    window.addEventListener(COMPARE_CHANGED_EVENT, sync)
    return () => window.removeEventListener(COMPARE_CHANGED_EVENT, sync)
  }, [id])

  const toggle = () => {
    setSelected(toggleCompare({ id, slug, name }))
  }

  return (
    <button
      type="button"
      onClick={toggle}
      aria-pressed={mounted ? selected : undefined}
      aria-label={`${selected ? 'Remove from' : 'Add to'} compare`}
      className={cn(
        'flex h-10 w-10 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background',
        className,
      )}
    >
      <Scale
        className={cn(
          'h-4 w-4 transition-colors duration-300',
          mounted && selected ? 'text-accent' : 'text-foreground',
        )}
      />
    </button>
  )
}

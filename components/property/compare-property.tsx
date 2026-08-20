'use client'

import { useEffect, useState } from 'react'
import { Scale } from 'lucide-react'

import { cn } from '@/lib/utils'
import { readCompare, toggleCompare } from '@/lib/compare'

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
    setSelected(readCompare().some((entry) => entry.id === id))
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
        'flex h-9 w-9 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background',
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

import Link from 'next/link'

import { SaveProperty } from '@/components/property/save-property'
import type { PropertyDetail } from '@/lib/property-types'

export function PropertyActions({ property }: { property: PropertyDetail }) {
  const tags = [
    ...(property.viewType ?? []),
  ].slice(0, 5)

  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-wrap gap-1.5">
        {tags.map((tag) => (
          <span
            key={tag}
            className="border border-border/80 px-2.5 py-1 text-[9px] font-light uppercase tracking-[0.14em] text-foreground/70"
          >
            {tag}
          </span>
        ))}
      </div>

      <div className="flex items-stretch gap-2">
        <Link
          href="/#contact"
          className="inline-flex min-h-11 flex-1 items-center justify-center bg-accent px-4 py-2.5 text-center text-[10px] font-medium uppercase tracking-[0.17em] text-accent-foreground shadow-sm transition-opacity duration-500 hover:opacity-90"
        >
          Book a Private Viewing
        </Link>

        <SaveProperty
          propertyId={property._id}
          className="min-h-11 flex-none px-4 py-2.5 text-[10px] tracking-[0.14em]"
        />
      </div>
    </div>
  )
}

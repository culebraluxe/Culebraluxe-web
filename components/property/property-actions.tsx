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
            className="border border-[#c6a15b]/35 bg-[#c6a15b]/[0.08] px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.14em] text-brand-navy/80"
          >
            {tag}
          </span>
        ))}
      </div>

      <div className="flex items-stretch gap-2">
        <Link
          href={`/contact?propertyId=${encodeURIComponent(property._id)}&requestType=private_viewing#contact`}
          className="inline-flex min-h-11 flex-1 items-center justify-center bg-brand-navy px-4 py-2.5 text-center text-[10px] font-medium uppercase tracking-[0.17em] text-[#f8f5ec] shadow-sm transition-colors duration-500 hover:bg-brand-navy"
        >
          Book a Private Viewing
        </Link>

        <SaveProperty
          propertyId={property._id}
          className="min-h-11 flex-none border-brand-navy/25 px-4 py-2.5 text-[10px] font-medium tracking-[0.14em] text-brand-navy hover:border-brand-navy/60"
        />
      </div>
    </div>
  )
}

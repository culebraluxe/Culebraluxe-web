'use client'

import Image from 'next/image'
import { useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import type { GalleryImage } from '@/lib/property-types'
import { cn } from '@/lib/utils'

export function PropertyGallery({ images }: { images: GalleryImage[] }) {
  const [active, setActive] = useState(0)

  if (images.length === 0) {
    return (
      <p className="text-sm font-light text-muted-foreground">
        Gallery imagery will be added shortly.
      </p>
    )
  }

  const current = images[active]
  const go = (dir: 1 | -1) =>
    setActive((i) => (i + dir + images.length) % images.length)

  return (
    <div className="flex flex-col gap-4">
      <div className="group relative aspect-[16/10] w-full overflow-hidden bg-muted">
        <Image
          key={current.url}
          src={current.url || '/placeholder.svg'}
          alt={current.alt}
          fill
          sizes="(min-width: 768px) 66vw, 100vw"
          className="object-cover"
        />

        {images.length > 1 && (
          <>
            <button
              type="button"
              onClick={() => go(-1)}
              aria-label="Previous image"
              className="absolute left-4 top-1/2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-background/80 text-foreground opacity-0 backdrop-blur-sm transition-opacity duration-300 hover:bg-background group-hover:opacity-100"
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={() => go(1)}
              aria-label="Next image"
              className="absolute right-4 top-1/2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-background/80 text-foreground opacity-0 backdrop-blur-sm transition-opacity duration-300 hover:bg-background group-hover:opacity-100"
            >
              <ChevronRight className="h-4 w-4" />
            </button>
            <div className="absolute bottom-4 right-4 bg-foreground/70 px-3 py-1 text-[11px] font-light uppercase tracking-[0.16em] text-background backdrop-blur-sm">
              {active + 1} / {images.length}
            </div>
          </>
        )}
      </div>

      {current.caption && (
        <p className="text-xs font-light uppercase tracking-[0.16em] text-muted-foreground">
          {current.caption}
        </p>
      )}

      {images.length > 1 && (
        <div className="grid grid-cols-4 gap-3 sm:grid-cols-6">
          {images.map((img, i) => (
            <button
              key={img.url}
              type="button"
              onClick={() => setActive(i)}
              aria-label={`View image ${i + 1}`}
              aria-current={i === active}
              className={cn(
                'relative aspect-square w-full overflow-hidden bg-muted transition-opacity duration-300',
                i === active
                  ? 'ring-1 ring-foreground ring-offset-2 ring-offset-background'
                  : 'opacity-70 hover:opacity-100',
              )}
            >
              <Image
                src={img.url || '/placeholder.svg'}
                alt=""
                fill
                sizes="120px"
                className="object-cover"
              />
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

'use client'

import Image from 'next/image'
import { useEffect, useState } from 'react'

import type { GalleryImage } from '@/lib/property-types'
import { cn } from '@/lib/utils'

type ActiveImage =
  | { kind: 'hero' }
  | { kind: 'gallery'; index: number }

export function PropertyMediaPanel({
  heroUrl,
  galleryImages,
  propertyTitle,
}: {
  heroUrl: string | null
  galleryImages: GalleryImage[]
  propertyTitle?: string | null
}) {
  const [activeImage, setActiveImage] = useState<ActiveImage>({
    kind: 'hero',
  })

  useEffect(() => {
    setActiveImage({ kind: 'hero' })
  }, [heroUrl, propertyTitle])

  let removedHero = false
  const supportingImages: {
    image: GalleryImage
    galleryIndex: number
  }[] = []

  galleryImages.forEach((image, galleryIndex) => {
    if (!removedHero && heroUrl && image.url === heroUrl) {
      removedHero = true
      return
    }

    supportingImages.push({ image, galleryIndex })
  })

  const visibleThumbnails = supportingImages.slice(0, 4)
  const remainingCount = supportingImages.length - visibleThumbnails.length
  const selectedGalleryImage =
    activeImage.kind === 'gallery'
      ? galleryImages[activeImage.index]
      : null
  const displayedUrl = selectedGalleryImage?.url ?? heroUrl
  const displayedAlt =
    selectedGalleryImage?.alt ?? propertyTitle ?? 'Property'

  return (
    <div className="absolute inset-0 flex flex-col gap-1 bg-brand-navy/[0.08] p-1">
      <div
        className={cn(
          'relative flex-none overflow-hidden bg-muted',
          visibleThumbnails.length > 0
            ? 'h-[210px] sm:h-[280px] lg:h-[342px]'
            : 'h-[282px] sm:h-[362px] lg:h-[424px]',
        )}
      >
        {displayedUrl && (
          <Image
            key={
              activeImage.kind === 'hero'
                ? 'hero'
                : `gallery-${activeImage.index}`
            }
            src={displayedUrl}
            alt={displayedAlt}
            fill
            priority={activeImage.kind === 'hero'}
            unoptimized
            sizes="100vw"
            className="object-cover"
          />
        )}

        {activeImage.kind !== 'hero' && heroUrl && (
          <button
            type="button"
            onClick={() => setActiveImage({ kind: 'hero' })}
            className="absolute right-3 top-3 z-10 border border-[#c6a15b]/45 bg-brand-navy/90 px-3 py-1.5 text-[9px] font-medium uppercase tracking-[0.16em] text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]/60"
            aria-label="Return to the original hero image"
          >
            Hero Image
          </button>
        )}
      </div>

      {visibleThumbnails.length > 0 && (
        <div
          className="grid h-[68px] flex-none grid-cols-4 gap-1 sm:h-[78px]"
          role="group"
          aria-label="Supporting property images"
        >
          {visibleThumbnails.map(({ image, galleryIndex }, index) => {
            const showRemaining =
              index === visibleThumbnails.length - 1 && remainingCount > 0
            const isSelected =
              activeImage.kind === 'gallery' &&
              activeImage.index === galleryIndex

            return (
              <button
                key={`${image.url}-${galleryIndex}`}
                type="button"
                onClick={() =>
                  setActiveImage({ kind: 'gallery', index: galleryIndex })
                }
                aria-label={`View ${image.alt || `property image ${index + 2}`}`}
                aria-current={isSelected ? 'true' : undefined}
                className={cn(
                  'relative h-[68px] overflow-hidden bg-muted transition-opacity sm:h-[78px] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-[#c6a15b]/70',
                  isSelected
                    ? 'opacity-100 ring-2 ring-inset ring-[#c6a15b]/70'
                    : 'opacity-85 hover:opacity-100',
                )}
              >
                <Image
                  src={image.url}
                  alt=""
                  fill
                  unoptimized
                  sizes="(min-width: 1024px) 16vw, 25vw"
                  className="object-cover"
                />

                {showRemaining && (
                  <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-brand-navy/80 text-sm font-medium uppercase tracking-[0.16em] text-[#f8f5ec] backdrop-blur-[1px]">
                    +{remainingCount}
                  </div>
                )}
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

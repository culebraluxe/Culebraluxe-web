'use client'

import Image from 'next/image'
import { useCallback, useEffect, useRef, useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'

import type { GalleryImage } from '@/lib/property-types'
import { cn } from '@/lib/utils'

export function PropertyMediaPanel({
  heroUrl,
  galleryImages,
  propertyTitle,
}: {
  heroUrl: string | null
  galleryImages: GalleryImage[]
  propertyTitle?: string | null
}) {
  const [activeIndex, setActiveIndex] = useState(0)
  const [navigationVisible, setNavigationVisible] = useState(true)
  const navigationTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearNavigationTimer = useCallback(() => {
    if (navigationTimerRef.current) {
      clearTimeout(navigationTimerRef.current)
      navigationTimerRef.current = null
    }
  }, [])

  const scheduleNavigationFade = useCallback(() => {
    clearNavigationTimer()

    if (!window.matchMedia('(hover: hover) and (pointer: fine)').matches) {
      setNavigationVisible(true)
      return
    }

    navigationTimerRef.current = setTimeout(() => {
      setNavigationVisible(false)
    }, 2800)
  }, [clearNavigationTimer])

  const revealNavigation = useCallback(() => {
    setNavigationVisible(true)
    scheduleNavigationFade()
  }, [scheduleNavigationFade])

  useEffect(() => {
    setActiveIndex(0)
  }, [heroUrl, propertyTitle])

  useEffect(() => {
    scheduleNavigationFade()
    return clearNavigationTimer
  }, [clearNavigationTimer, scheduleNavigationFade])

  const heroImage = galleryImages.find((image) => image.url === heroUrl)
  const mediaImages: GalleryImage[] = []

  if (heroUrl) {
    mediaImages.push({
      url: heroUrl,
      alt: heroImage?.alt ?? propertyTitle ?? 'Property',
      caption: heroImage?.caption ?? null,
    })
  }

  let removedHero = false
  for (const image of galleryImages) {
    if (!removedHero && heroUrl && image.url === heroUrl) {
      removedHero = true
      continue
    }
    mediaImages.push(image)
  }

  const normalizedActiveIndex =
    mediaImages.length > 0 ? activeIndex % mediaImages.length : 0
  const activeImage = mediaImages[normalizedActiveIndex] ?? null
  const supportingImages: {
    image: GalleryImage
    mediaIndex: number
  }[] = []

  mediaImages.forEach((image, mediaIndex) => {
    if (mediaIndex === 0) return
    supportingImages.push({ image, mediaIndex })
  })

  const visibleThumbnails = supportingImages.slice(0, 4)
  const remainingCount = supportingImages.length - visibleThumbnails.length
  const displayedUrl = activeImage?.url ?? null
  const displayedAlt = activeImage?.alt ?? propertyTitle ?? 'Property'
  const hasNavigation = mediaImages.length > 1

  function showPreviousImage() {
    if (!hasNavigation) return
    setActiveIndex(
      (current) => (current - 1 + mediaImages.length) % mediaImages.length,
    )
  }

  function showNextImage() {
    if (!hasNavigation) return
    setActiveIndex((current) => (current + 1) % mediaImages.length)
  }

  return (
    <div
      className="absolute inset-0 flex flex-col gap-1 bg-brand-navy/[0.08] p-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#c6a15b]/70"
      tabIndex={0}
      role="group"
      aria-label="Property photo gallery"
      onPointerMove={revealNavigation}
      onPointerDown={revealNavigation}
      onFocusCapture={() => {
        clearNavigationTimer()
        setNavigationVisible(true)
      }}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          scheduleNavigationFade()
        }
      }}
      onKeyDown={(event) => {
        if (event.key === 'ArrowLeft') {
          event.preventDefault()
          showPreviousImage()
        } else if (event.key === 'ArrowRight') {
          event.preventDefault()
          showNextImage()
        }
      }}
    >
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
              normalizedActiveIndex === 0
                ? 'hero'
                : `gallery-${normalizedActiveIndex}`
            }
            src={displayedUrl}
            alt={displayedAlt}
            fill
            priority={normalizedActiveIndex === 0}
            unoptimized
            sizes="100vw"
            className="object-cover"
          />
        )}

        {normalizedActiveIndex !== 0 && heroUrl && (
          <button
            type="button"
            onClick={() => setActiveIndex(0)}
            className="absolute right-3 top-3 z-10 border border-[#c6a15b]/45 bg-brand-navy/90 px-3 py-1.5 text-[9px] font-medium uppercase tracking-[0.16em] text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]/60"
            aria-label="Return to the original hero image"
          >
            Hero Image
          </button>
        )}

        {hasNavigation && (
          <div
            className={cn(
              'pointer-events-none absolute inset-0 z-10 transition-opacity duration-300',
              navigationVisible
                ? 'opacity-100'
                : 'opacity-0',
            )}
          >
            <button
              type="button"
              onClick={showPreviousImage}
              aria-label="Previous photo"
              className={cn(
                'absolute left-2 top-1/2 flex h-12 w-12 -translate-y-1/2 items-center justify-center rounded-full border border-[#c6a15b]/30 bg-brand-navy/55 text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]',
                navigationVisible && 'pointer-events-auto',
              )}
            >
              <ChevronLeft className="h-5 w-5" aria-hidden />
            </button>

            <button
              type="button"
              onClick={showNextImage}
              aria-label="Next photo"
              className={cn(
                'absolute right-2 top-1/2 flex h-12 w-12 -translate-y-1/2 items-center justify-center rounded-full border border-[#c6a15b]/30 bg-brand-navy/55 text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]',
                navigationVisible && 'pointer-events-auto',
              )}
            >
              <ChevronRight className="h-5 w-5" aria-hidden />
            </button>
          </div>
        )}

        {mediaImages.length > 0 && (
          <p
            className="absolute bottom-3 right-3 z-10 bg-brand-navy/70 px-2.5 py-1 text-[10px] font-medium tabular-nums tracking-[0.12em] text-[#f8f5ec] backdrop-blur-sm"
            aria-live="polite"
          >
            {normalizedActiveIndex + 1} / {mediaImages.length}
          </p>
        )}
      </div>

      {visibleThumbnails.length > 0 && (
        <div
          className="grid h-[68px] flex-none grid-cols-4 gap-1 sm:h-[78px]"
          role="group"
          aria-label="Supporting property images"
        >
          {visibleThumbnails.map(({ image, mediaIndex }, index) => {
            const showRemaining =
              index === visibleThumbnails.length - 1 && remainingCount > 0
            const isSelected =
              normalizedActiveIndex === mediaIndex

            return (
              <button
                key={`${image.url}-${mediaIndex}`}
                type="button"
                onClick={() => setActiveIndex(mediaIndex)}
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

'use client'

import Image from 'next/image'
import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ChevronLeft, ChevronRight, Images, X } from 'lucide-react'

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
  const [lightboxOpen, setLightboxOpen] = useState(false)
  const [lightboxIndex, setLightboxIndex] = useState(0)
  const closeButtonRef = useRef<HTMLButtonElement | null>(null)

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

  const mediaImagesRef = useRef(mediaImages)
  mediaImagesRef.current = mediaImages

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

  // Lightbox "View All Photos" — shows every media image in canonical order.
  const lightboxTotal = mediaImages.length
  const lightboxImage =
    lightboxTotal > 0 ? mediaImages[lightboxIndex % lightboxTotal] ?? null : null
  const firstHiddenMediaIndex =
    supportingImages[visibleThumbnails.length]?.mediaIndex ?? 0

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

  function openLightbox(index: number) {
    if (mediaImages.length === 0) return
    setLightboxIndex(index % mediaImages.length)
    setLightboxOpen(true)
  }

  function moveLightbox(delta: 1 | -1) {
    const total = mediaImagesRef.current.length
    if (total === 0) return
    setLightboxIndex((current) => (current + delta + total) % total)
  }

  useEffect(() => {
    if (!lightboxOpen) return

    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    closeButtonRef.current?.focus()

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault()
        setLightboxOpen(false)
      } else if (event.key === 'ArrowLeft') {
        event.preventDefault()
        moveLightbox(-1)
      } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        moveLightbox(1)
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => {
      document.body.style.overflow = previousOverflow
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [lightboxOpen])

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

        {mediaImages.length > 1 && (
          <button
            type="button"
            onClick={() => openLightbox(normalizedActiveIndex)}
            className="absolute bottom-3 left-3 z-10 inline-flex min-h-11 items-center gap-2 border border-[#c6a15b]/45 bg-brand-navy/80 px-3.5 py-1.5 text-[10px] font-medium uppercase tracking-[0.16em] text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]/60"
            aria-label="View all photos"
          >
            View all photos
            <Images className="h-3.5 w-3.5" aria-hidden />
          </button>
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
                onClick={
                  showRemaining
                    ? () => openLightbox(firstHiddenMediaIndex)
                    : () => setActiveIndex(mediaIndex)
                }
                aria-label={
                  showRemaining
                    ? 'View all photos'
                    : `View ${image.alt || `property image ${index + 2}`}`
                }
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

      {lightboxOpen &&
        createPortal(
          <div
            role="dialog"
            aria-modal="true"
            aria-label="All photos"
            className="fixed inset-0 z-[70] flex flex-col bg-brand-navy/95 text-[#f8f5ec]"
          >
            <header className="flex items-center justify-between gap-4 px-4 py-3 sm:px-6">
              <p className="text-xs font-medium uppercase tracking-[0.24em] text-[#c6a15b]">
                {lightboxTotal > 0
                  ? `${(lightboxIndex % lightboxTotal) + 1} / ${lightboxTotal}`
                  : ''}
              </p>

              <button
                ref={closeButtonRef}
                type="button"
                onClick={() => setLightboxOpen(false)}
                className="inline-flex min-h-12 min-w-12 items-center justify-center rounded-full border border-[#c6a15b]/40 bg-brand-navy/60 text-[#f8f5ec] shadow-sm transition hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]"
                aria-label="Close photo viewer"
              >
                <X className="h-5 w-5" aria-hidden />
              </button>
            </header>

            <div className="relative min-h-0 flex-1">
              {lightboxImage && (
                <Image
                  key={`lightbox-${lightboxIndex % lightboxTotal}`}
                  src={lightboxImage.url}
                  alt={lightboxImage.alt}
                  fill
                  unoptimized
                  sizes="100vw"
                  className="object-contain"
                />
              )}

              {lightboxTotal > 1 && (
                <>
                  <button
                    type="button"
                    onClick={() => moveLightbox(-1)}
                    aria-label="Previous photo"
                    className="absolute left-2 top-1/2 flex h-14 w-14 -translate-y-1/2 items-center justify-center rounded-full border border-[#c6a15b]/40 bg-brand-navy/70 text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]"
                  >
                    <ChevronLeft className="h-7 w-7" aria-hidden />
                  </button>

                  <button
                    type="button"
                    onClick={() => moveLightbox(1)}
                    aria-label="Next photo"
                    className="absolute right-2 top-1/2 flex h-14 w-14 -translate-y-1/2 items-center justify-center rounded-full border border-[#c6a15b]/40 bg-brand-navy/70 text-[#f8f5ec] shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#c6a15b]"
                  >
                    <ChevronRight className="h-7 w-7" aria-hidden />
                  </button>
                </>
              )}
            </div>

            {lightboxTotal > 1 && (
              <div className="flex gap-2 overflow-x-auto px-4 py-4 sm:px-6">
                {mediaImages.map((image, index) => {
                  const selected = index === lightboxIndex % lightboxTotal

                  return (
                    <button
                      key={`${image.url}-${index}`}
                      type="button"
                      onClick={() => setLightboxIndex(index)}
                      aria-label={`View ${image.alt || `photo ${index + 1}`}`}
                      aria-current={selected ? 'true' : undefined}
                      className={cn(
                        'relative h-16 w-24 flex-none overflow-hidden rounded-sm bg-brand-navy transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#c6a15b]',
                        selected
                          ? 'ring-2 ring-inset ring-[#c6a15b]'
                          : 'opacity-70 hover:opacity-100',
                      )}
                    >
                      <Image
                        src={image.url}
                        alt=""
                        fill
                        unoptimized
                        sizes="96px"
                        className="object-cover"
                      />
                    </button>
                  )
                })}
              </div>
            )}
          </div>,
          document.body,
        )}
    </div>
  )
}

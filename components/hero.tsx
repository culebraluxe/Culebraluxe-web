import Image from 'next/image'
import type { MarketingContentBlock } from '@/lib/marketing-content'

export function Hero({ content }: { content: MarketingContentBlock }) {
  return (
    <section id="top" className="relative h-[100svh] w-full overflow-hidden">
      <Image
        src={content.imagePath ?? '/images/hero-villa.png'}
        alt={
          content.imageAlt ??
          'Cliffside modern villa overlooking the turquoise Caribbean sea in Culebra'
        }
        fill
        priority
        sizes="100vw"
        className="object-cover"
      />
      {/* Cinematic gradient scrims for legibility */}
      <div className="absolute inset-0 bg-gradient-to-b from-black/40 via-black/10 to-black/50" />
      <div className="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent" />

      <div className="relative flex h-full flex-col justify-end px-6 pb-20 md:px-12 md:pb-28">
        <div className="mx-auto w-full max-w-[1600px]">
          <p className="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/70 animate-[fadeUp_1.2s_ease-out_both]">
            {content.eyebrow}
          </p>
          <h1 className="max-w-4xl text-balance font-serif text-5xl font-light leading-[1.02] text-background md:text-7xl lg:text-8xl animate-[fadeUp_1.2s_ease-out_0.15s_both]">
            {content.title}
          </h1>
          <div className="mt-10 flex flex-col gap-6 border-t border-background/25 pt-8 md:flex-row md:items-end md:justify-between animate-[fadeUp_1.2s_ease-out_0.35s_both]">
            <p className="max-w-md text-pretty text-sm font-light leading-relaxed text-background/80">
              {content.body}
            </p>
            <a
              href={content.ctaHref ?? '#properties'}
              className="group inline-flex items-center gap-3 self-start text-xs font-light uppercase tracking-[0.28em] text-background transition-colors md:self-auto"
            >
              {content.ctaLabel}
              <span className="inline-block h-px w-10 bg-background transition-all duration-500 group-hover:w-16" />
            </a>
          </div>
        </div>
      </div>
    </section>
  )
}

import Image from 'next/image'
import { Reveal } from '@/components/reveal'
import { itemsFor, type MarketingContentBlock } from '@/lib/marketing-content'

export function Services({
  buyers,
  sellers,
}: {
  buyers: MarketingContentBlock
  sellers: MarketingContentBlock
}) {
  const buyerItems = itemsFor(buyers, 'list')

  return (
    <div className="bg-primary text-primary-foreground">
      {/* Buyers */}
      <section id="buyers" className="border-b border-primary-foreground/10 px-6 py-28 md:px-12 md:py-40">
        <div className="mx-auto grid max-w-[1600px] gap-14 md:grid-cols-2 md:gap-24">
          <Reveal>
            <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
              {buyers.eyebrow}
            </p>
            <h2 className="text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl">
              {buyers.title}
            </h2>
          </Reveal>
          <Reveal delay={120} className="flex flex-col justify-center gap-10">
            <p className="max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75">
              {buyers.body}
            </p>
            <ul className="flex flex-col divide-y divide-primary-foreground/10 border-y border-primary-foreground/10">
              {buyerItems.map((item) => (
                <li
                  key={item.value}
                  className="py-5 text-sm font-light tracking-wide text-primary-foreground/85"
                >
                  {item.value}
                </li>
              ))}
            </ul>
            <a
              href={buyers.ctaHref ?? '#contact'}
              className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
            >
              {buyers.ctaLabel}
              <span className="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16" />
            </a>
          </Reveal>
        </div>
      </section>

      {/* Sellers */}
      <section id="sellers" className="px-6 py-28 md:px-12 md:py-40">
        <div className="mx-auto grid max-w-[1600px] items-center gap-14 md:grid-cols-2 md:gap-24">
          <Reveal className="order-2 md:order-1">
            <div className="relative aspect-[4/5] w-full overflow-hidden">
              <Image
                src={sellers.imagePath ?? '/images/coastline.png'}
                alt={
                  sellers.imageAlt ??
                  'Aerial view of the Culebra coastline with jade and turquoise water'
                }
                fill
                sizes="(min-width: 768px) 50vw, 100vw"
                className="object-cover"
              />
            </div>
          </Reveal>
          <Reveal delay={120} className="order-1 flex flex-col gap-10 md:order-2">
            <div>
              <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
                {sellers.eyebrow}
              </p>
              <h2 className="text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl">
                {sellers.title}
              </h2>
            </div>
            <p className="max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75">
              {sellers.body}
            </p>
            <a
              href={sellers.ctaHref ?? '#contact'}
              className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
            >
              {sellers.ctaLabel}
              <span className="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16" />
            </a>
          </Reveal>
        </div>
      </section>
    </div>
  )
}

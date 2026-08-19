import type { Metadata } from 'next'
import Link from 'next/link'

import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { SiteFooter } from '@/components/site-footer'
import { SiteHeader } from '@/components/site-header'
import { getGuideItems } from '@/db/guide'

export const metadata: Metadata = {
  title: 'Island Guide — CulebraLuxe',
  description:
    'A quiet guide to life on Culebra — its beaches, marine reserves, and the unhurried rhythm that rewards those who arrive attentively.',
}

const GUIDE_SECTIONS = [
  {
    id: 'beaches',
    number: '01',
    title: 'BEACHES',
    headline: 'The edges of the island.',
    description:
      'From world-famous shores to quiet hidden coves, every beach in Culebra has its own character.',
  },
  {
    id: 'water',
    number: '02',
    title: 'WATER',
    headline: 'The island from the water.',
    description:
      'Reefs, protected bays, open water and surrounding cays make the sea part of everyday life on Culebra.',
  },
  {
    id: 'wildlife-land',
    number: '03',
    title: 'WILDLIFE & LAND',
    headline: 'A landscape worth protecting.',
    description:
      'Refuge lands, dry forest, trails and coastal habitat reveal the quieter natural side of the island.',
  },
  {
    id: 'coffee-casual',
    number: '04',
    title: 'COFFEE & CASUAL',
    headline: 'Easy mornings and simple stops.',
    description:
      'Coffee, breakfast, beach kiosks and casual favorites for the unhurried rhythm of island days.',
  },
  {
    id: 'dining',
    number: '05',
    title: 'DINING',
    headline: 'Where the island gathers.',
    description:
      'Waterfront tables, local seafood, pizza, tacos and relaxed evening spots across Culebra.',
  },
  {
    id: 'getting-here',
    number: '06',
    title: 'GETTING HERE',
    headline: 'Arriving is part of the experience.',
    description:
      'Flights and ferry service connect Culebra with San Juan, Ceiba and Puerto Rico’s main island.',
  },
  {
    id: 'getting-around',
    number: '07',
    title: 'GETTING AROUND',
    headline: 'Small island, easy rhythm.',
    description:
      'Jeeps, carts and local taxis make it simple to move between town, beaches and the hills.',
  },
  {
    id: 'essentials',
    number: '08',
    title: 'ISLAND ESSENTIALS',
    headline: 'The practical side of island life.',
    description:
      'Groceries, medical care, banking, hardware and everyday services that keep life on Culebra moving.',
  },
  {
    id: 'island-story',
    number: '09',
    title: 'ISLAND STORY',
    headline: 'An island shaped by history.',
    description:
      'Indigenous roots, Spanish rule, military history, community resistance and conservation all shaped modern Culebra.',
  },
]

export const dynamic = 'force-dynamic'

export default async function GuidePage() {
  const guideItems = await getGuideItems()

  return (
    <>
      <SiteHeader />

      <main>
        <PageHero
          eyebrow="Island Guide"
          title="A slower rhythm, kept intentionally intact."
          intro="No traffic lights. No high-rises. Fishing boats at dawn, reef-clear water by noon, and evenings measured in shades of gold."
          image="/images/culture.png"
          imageAlt="The white sand crescent and turquoise water of Flamenco Beach, Culebra"
        />

        {/* Guide navigation */}
        <section className="border-b border-border px-6 md:px-12">
          <div className="mx-auto max-w-[1600px] overflow-x-auto">
            <nav className="flex min-w-max gap-8 py-6 md:gap-10">
              {GUIDE_SECTIONS.map((section) => (
                <a
                  key={section.id}
                  href={`#${section.id}`}
                  className="text-[11px] font-light uppercase tracking-[0.22em] text-muted-foreground transition-colors hover:text-foreground"
                >
                  {section.title}
                </a>
              ))}
            </nav>
          </div>
        </section>

        {/* Guide sections */}
        <section className="px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <div className="space-y-24 md:space-y-32">
              {GUIDE_SECTIONS.map((section, i) => (
                <section
                  key={section.id}
                  id={section.id}
                  className="scroll-mt-24"
                >
                  <Reveal delay={i * 60}>
                    <div className="grid gap-10 md:grid-cols-12 md:gap-12">
                      <div className="md:col-span-3">
                        <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                          {section.number} / {section.title}
                        </p>

                        <h2 className="mt-5 font-serif text-2xl font-light leading-[1.2] text-foreground md:text-3xl">
                          {section.headline}
                        </h2>

                        <p className="mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                          {section.description}
                        </p>
                      </div>

                      <div className="min-w-0 md:col-span-9">
                        <div className="flex gap-5 overflow-x-auto pb-4">
                          {guideItems
                            .filter((item) => item.section === section.id)
                            .map((item) => (
                              <article
                                key={item.id}
                                className="w-[78vw] max-w-[280px] shrink-0 sm:w-[240px] lg:w-[220px]"
                              >
                                <div className="aspect-[4/3] overflow-hidden bg-muted">
                                  {item.imageUrl ? (
                                    <img
                                      src={item.imageUrl}
                                      alt={item.imageAlt ?? item.name}
                                      className="h-full w-full object-cover"
                                    />
                                  ) : (
                                    <div className="flex h-full items-center justify-center">
                                      <span className="text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
                                        Image coming soon
                                      </span>
                                    </div>
                                  )}
                                </div>

                                <div className="pt-5">
                                  <p className="text-[10px] font-light uppercase tracking-[0.22em] text-accent">
                                    {item.subtitle || item.area || item.eyebrow}
                                  </p>

                                  <h3 className="mt-2 font-serif text-xl font-light leading-tight text-foreground">
                                    {item.name}
                                  </h3>

                                  <p className="mt-3 text-sm font-light leading-relaxed text-muted-foreground">
                                    {item.description}
                                  </p>

                                  {item.address && (
                                    <p className="mt-4 text-xs font-light leading-relaxed text-muted-foreground">
                                      {item.address}
                                    </p>
                                  )}

                                  {item.phone && (
                                    <a
                                      href={`tel:${item.phone}`}
                                      className="mt-2 block text-xs font-light text-foreground"
                                    >
                                      {item.phone}
                                    </a>
                                  )}

                                  {item.websiteUrl && (
                                    <a
                                      href={item.websiteUrl}
                                      target="_blank"
                                      rel="noreferrer"
                                      className="mt-3 inline-block text-[10px] font-light uppercase tracking-[0.2em] text-foreground"
                                    >
                                      Visit website →
                                    </a>
                                  )}
                                </div>
                              </article>
                            ))}
                        </div>
                      </div>
                    </div>
                  </Reveal>
                </section>
              ))}
            </div>
          </div>
        </section>

        {/* CTA */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
          <div className="mx-auto flex max-w-[1600px] flex-col items-start gap-8">
            <Reveal>
              <h2 className="max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                When you are ready to find your place here.
              </h2>
            </Reveal>

            <Reveal delay={120}>
              <Link
                href="/buyers"
                className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
              >
                Explore buying on Culebra
                <span className="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16" />
              </Link>
            </Reveal>
          </div>
        </section>
      </main>

      <SiteFooter />
    </>
  )
}
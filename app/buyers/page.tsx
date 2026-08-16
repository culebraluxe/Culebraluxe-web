import type { Metadata } from 'next'
import Link from 'next/link'

import { getProperties } from '@/db/properties'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { BuyersPropertyShowroom } from '@/components/buyers-property-showroom'

export const metadata: Metadata = {
  title: 'For Buyers — CulebraLuxe',
  description:
    'Explore exceptional homes, villas, and land for sale on Culebra, Puerto Rico.',
}

const STEPS = [
  {
    n: '01',
    title: 'A quiet conversation',
    body: 'We begin by understanding what you are truly seeking — the light, the outlook, the rhythm of days. No pressure, no listings sheet. Just a considered discussion of possibility.',
  },
  {
    n: '02',
    title: 'Private viewings',
    body: 'Many of the finest homes on Culebra never reach a public listing. We arrange discreet, unhurried viewings — including off-market residences held within our private network.',
  },
  {
    n: '03',
    title: 'Diligence & title',
    body: 'We coordinate title research, survey review, and legal counsel, translating the particulars of Puerto Rico property law into clear, unhurried guidance.',
  },
  {
    n: '04',
    title: 'Closing & beyond',
    body: 'From closing logistics to introductions for architects, builders, and island life, we remain a steady presence well after the keys change hands.',
  },
]

const SERVICES = [
  'Private, unlisted viewings',
  'Legal, title & closing guidance',
  'Architecture & renovation introductions',
  'Residency & relocation support',
  'Property management referrals',
  'Long-term stewardship advice',
]

export default async function BuyersPage() {
  const properties = await getProperties()

  return (
    <>
      <SiteHeader />

      <main>
        <PageHero
          eyebrow="For Buyers"
          title="Find your place on Culebra."
          intro="Exceptional homes, villas, and land — presented with the perspective of people who know the island intimately."
          image="/images/hero-villa.png"
          imageAlt="A modern luxury villa overlooking the Culebra coastline"
        />

        <BuyersPropertyShowroom properties={properties} />

        {/* Buying guidance now supports the inventory rather than blocking it */}
        <section className="px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <div className="mb-16 max-w-3xl md:mb-20">
                <p className="mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent">
                  Buying on Culebra
                </p>

                <h2 className="text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                  A considered path from first look to ownership.
                </h2>

                <p className="mt-6 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                  Finding the right property is only the beginning.
                  We guide the details that follow — privately,
                  carefully, and with an understanding of how
                  transactions work on the island.
                </p>
              </div>
            </Reveal>

            <div className="grid gap-12 md:grid-cols-2 lg:grid-cols-4 lg:gap-8">
              {STEPS.map((step, index) => (
                <Reveal
                  key={step.n}
                  delay={index * 80}
                >
                  <div className="border-t border-border pt-7">
                    <span className="font-serif text-2xl font-light text-accent">
                      {step.n}
                    </span>

                    <h3 className="mt-7 font-serif text-2xl font-light leading-snug text-foreground">
                      {step.title}
                    </h3>

                    <p className="mt-4 text-sm font-light leading-relaxed text-muted-foreground">
                      {step.body}
                    </p>
                  </div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* Private / off-market moment */}
        <section className="bg-foreground px-6 py-24 text-background md:px-12 md:py-32">
          <div className="mx-auto grid max-w-[1600px] gap-14 lg:grid-cols-12 lg:items-center">
            <Reveal className="lg:col-span-7">
              <p className="mb-5 text-xs font-light uppercase tracking-[0.34em] text-background/50">
                Private Opportunities
              </p>

              <h2 className="max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] md:text-5xl lg:text-6xl">
                Not every exceptional property is publicly listed.
              </h2>
            </Reveal>

            <Reveal
              delay={100}
              className="lg:col-span-4 lg:col-start-9"
            >
              <p className="text-sm font-light leading-relaxed text-background/70">
                Culebra remains a small island with a highly
                relationship-driven property market. Some owners
                prefer discretion. Tell us what you are looking for,
                and we can widen the search beyond the public inventory.
              </p>

              <Link
                href="/contact"
                className="group mt-8 inline-flex items-center gap-3 border border-background/30 px-8 py-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-500 hover:border-background"
              >
                Begin a private search

                <span className="inline-block h-px w-8 bg-background transition-all duration-500 group-hover:w-12" />
              </Link>
            </Reveal>
          </div>
        </section>

        {/* Supporting services */}
        <section className="px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto grid max-w-[1600px] gap-14 lg:grid-cols-12 lg:gap-20">
            <Reveal className="lg:col-span-5">
              <p className="mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent">
                Beyond the Search
              </p>

              <h2 className="max-w-lg text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                Every detail, quietly handled.
              </h2>
            </Reveal>

            <Reveal
              delay={100}
              className="lg:col-span-6 lg:col-start-7"
            >
              <ul className="flex flex-col divide-y divide-border border-y border-border">
                {SERVICES.map((item) => (
                  <li
                    key={item}
                    className="py-5 text-sm font-light tracking-wide text-foreground/80"
                  >
                    {item}
                  </li>
                ))}
              </ul>
            </Reveal>
          </div>
        </section>
      </main>

      <SiteFooter />
    </>
  )
}
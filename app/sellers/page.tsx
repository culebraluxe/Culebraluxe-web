import type { Metadata } from 'next'
import Image from 'next/image'
import Link from 'next/link'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'

export const metadata: Metadata = {
  title: 'For Sellers — CulebraLuxe',
  description:
    'An editorial approach to selling your Culebra home — considered photography, measured storytelling, and a private network of international buyers.',
}

const PILLARS = [
  {
    title: 'Editorial presentation',
    body: 'Considered photography, film, and writing that render your home the way it is actually lived in — not a checklist of features, but a sense of place.',
  },
  {
    title: 'A private network',
    body: 'We introduce your home to a discreet circle of international buyers who understand Culebra and are prepared to become its next stewards.',
  },
  {
    title: 'Measured guidance',
    body: 'From valuation and timing to negotiation and closing, we advise with candor and patience, never volume for its own sake.',
  },
]

export default function SellersPage() {
  return (
    <>
      <SiteHeader />
      <main>
        <PageHero
          eyebrow="For Sellers"
          title="Presented to the few who truly belong here."
          intro="Your home deserves more than a listing. We craft an editorial presentation — considered photography, measured storytelling, and introductions to a private network of international buyers who understand Culebra."
          image="/images/coastline.png"
          imageAlt="Aerial view of the Culebra coastline with jade and turquoise water"
        />

        {/* Pillars */}
        <section className="px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <p className="mb-16 text-xs font-light uppercase tracking-[0.34em] text-accent md:mb-24">
                Our Approach
              </p>
            </Reveal>
            <div className="grid gap-14 border-t border-border pt-16 md:grid-cols-3 md:gap-16">
              {PILLARS.map((pillar, i) => (
                <Reveal key={pillar.title} delay={i * 120}>
                  <h2 className="font-serif text-2xl font-light leading-snug text-foreground">
                    {pillar.title}
                  </h2>
                  <p className="mt-4 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                    {pillar.body}
                  </p>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* Image + valuation CTA */}
        <section className="px-6 pb-24 md:px-12 md:pb-32">
          <div className="mx-auto grid max-w-[1600px] items-center gap-14 md:grid-cols-2 md:gap-24">
            <Reveal>
              <div className="relative aspect-[4/5] w-full overflow-hidden">
                <Image
                  src="/images/hero-villa.png"
                  alt="A luxury Culebra residence with infinity pool overlooking the sea"
                  fill
                  sizes="(min-width: 768px) 50vw, 100vw"
                  className="object-cover"
                />
              </div>
            </Reveal>
            <Reveal delay={120} className="flex flex-col gap-10">
              <h2 className="text-balance font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                Begin with a private valuation.
              </h2>
              <p className="max-w-md text-pretty text-sm font-light leading-relaxed text-muted-foreground">
                We will spend time with your home, understand its story, and prepare a
                considered assessment of its position in today&apos;s market — without
                obligation, and always in confidence.
              </p>
              <Link
                href="/contact"
                className="group inline-flex items-center gap-3 self-start text-xs font-light uppercase tracking-[0.24em] text-foreground"
              >
                Request a valuation
                <span className="inline-block h-px w-10 bg-accent transition-all duration-500 group-hover:w-16" />
              </Link>
            </Reveal>
          </div>
        </section>
      </main>
      <SiteFooter />
    </>
  )
}

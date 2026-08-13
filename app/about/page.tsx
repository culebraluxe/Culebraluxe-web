import type { Metadata } from 'next'
import Link from 'next/link'

import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { SiteFooter } from '@/components/site-footer'
import { SiteHeader } from '@/components/site-header'
import { Waves, Palmtree, Leaf } from 'lucide-react'

export const metadata: Metadata = {
  title: 'About Us — CulebraLuxe',
  description:
    'CulebraLuxe is a boutique brokerage devoted to a single island — working with few clients, few homes, and an uncommon amount of care.',
}

const VALUES = [
  {
    title: 'Fit over volume',
    body: 'We measure success not in transactions but in fit — pairing the right stewards with the right homes.',
    icon: Waves,
  },
  {
    title: 'Local, truly',
    body: 'Founded by island residents, we know Culebra beyond its coordinates — the trade winds, the tide charts, and the people who shape it.',
    icon: Palmtree,
  },
  {
    title: 'Quiet stewardship',
    body: 'We protect the character that makes this place rare, advising with discretion and patience at every turn.',
    icon: Leaf,
  },
]

const REASONS = [
  {
    title: 'Boutique by design',
    body: 'We intentionally work with a limited number of clients.',
  },
  {
    title: 'Island-specific expertise',
    body: 'We understand unique homes, land, waterfront, and the realities of island ownership.',
  },
  {
    title: 'Personally handled',
    body: 'Every search, showing, and negotiation is handled directly and deliberately.',
  },
  {
    title: 'Trusted by referral',
    body: 'Much of our work comes through personal introductions and word of mouth.',
  },
]

const STATS = [
  { k: '14', v: 'Years on island' },
  { k: '1', v: 'Island, entirely' },
  { k: '40+', v: 'Homes stewarded' },
  { k: '100%', v: 'By referral' },
]

const LIFE_IMAGES = [
  '/images/about/life-01.jpg',
  '/images/about/life-02.jpg',
  '/images/about/life-03.jpg',
  '/images/about/life-04.jpg',
  '/images/about/life-05.jpg',
  '/images/about/life-06.jpg',
  '/images/about/life-07.jpg',
  '/images/about/life-08.jpg',
]

export default function AboutPage() {
  return (
    <>
      <SiteHeader />

      <main>
        <PageHero
          eyebrow="About Us"
          title="Devoted to a single island."
          intro="CulebraLuxe is a boutique brokerage working with few clients, few homes, and an uncommon amount of care."
          image="/images/about/about-hero.jpg"
          imageAlt="Aerial view across Culebra and the surrounding Caribbean water"
        />

        {/* Founder */}
        <section className="px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid items-stretch gap-12 md:grid-cols-2 md:gap-0">
              <Reveal>
                <div className="h-full min-h-[520px] overflow-hidden bg-muted md:min-h-[680px]">
                  <img
                    src="/images/about/lisa-portrait.jpg"
                    alt="Lisa Penfield, founder and broker of CulebraLuxe"
                    className="h-full w-full object-cover"
                  />
                </div>
              </Reveal>

              <Reveal delay={120}>
                <div className="flex h-full flex-col justify-center px-0 py-4 md:px-16 lg:px-20">
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    Lisa Penfield
                  </p>

                  <h2 className="mt-5 font-serif text-4xl font-light leading-[1.1] text-foreground md:text-5xl">
                    Founder & Broker
                  </h2>

                  <div className="mt-8 max-w-xl space-y-5 text-sm font-light leading-relaxed text-muted-foreground">
                    <p>
                      Lisa Penfield has spent more than two decades reading Culebra — first
                      as a world-champion windsurfer studying its wind and water, later as
                      a broker studying its land and light.
                    </p>

                    <p>
                      The same discipline that wins a championship applies just as well to
                      representing a home: patience, precision, and knowing exactly when
                      to act.
                    </p>

                    <p>
                      Licensed in Puerto Rico and a full-time resident of the island, she
                      works with a deliberately small number of clients each year — enough
                      to give every search, listing, and negotiation her full attention.
                    </p>
                  </div>

                  <p className="mt-8 font-serif text-xl font-light italic text-accent">
                    “Here, it’s personal. Always has been.”
                  </p>

                  <div className="mt-10 divide-y divide-border border-y border-border">
                    {[
                      'Former Windsurfing World Champion',
                      'Licensed Puerto Rico Real Estate Broker',
                      'Full-time Culebra resident',
                      'Referral-led, boutique practice',
                    ].map((item) => (
                      <div
                        key={item}
                        className="flex items-center gap-4 py-4"
                      >
                        <span className="text-accent">○</span>
                        <p className="text-sm font-light text-foreground">
                          {item}
                        </p>
                      </div>
                    ))}
                  </div>
                </div>
              </Reveal>
            </div>
          </div>
        </section>

     {/* Values */}
<section className="px-6 py-20 md:px-12 md:py-24">
  <div className="mx-auto max-w-[1600px]">
    <Reveal>
      <h2 className="text-center font-serif text-3xl font-light text-foreground md:text-4xl">
        What we value
      </h2>
    </Reveal>

    <div className="mt-14 grid gap-14 md:grid-cols-3 md:gap-0">
      {VALUES.map((value, i) => {
        const Icon = value.icon

        return (
          <Reveal key={value.title} delay={i * 100}>
            <div className="px-4 text-center md:border-r md:border-border md:px-12 last:md:border-r-0">
              
              <div className="mx-auto mb-6 flex h-12 w-12 items-center justify-center text-accent">
                <Icon
                  className="h-10 w-10"
                  strokeWidth={1.25}
                />
              </div>

              <h3 className="text-sm font-medium uppercase tracking-[0.16em] text-foreground">
                {value.title}
              </h3>

              <p className="mx-auto mt-4 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                {value.body}
              </p>
            </div>
          </Reveal>
        )
      })}
    </div>
  </div>
</section>

        {/* Why clients choose us */}
        <section className="bg-muted/30 px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid items-center gap-12 md:grid-cols-2 md:gap-16">
              <Reveal>
                <div>
                  <h2 className="font-serif text-3xl font-light leading-tight text-foreground md:text-4xl">
                    Why clients choose CulebraLuxe
                  </h2>

                  <div className="mt-10 space-y-7">
                    {REASONS.map((reason) => (
                      <div
                        key={reason.title}
                        className="flex gap-4"
                      >
                        <div className="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-accent text-xs text-accent">
                          ✓
                        </div>

                        <p className="text-sm font-light leading-relaxed text-muted-foreground">
                          <span className="font-medium text-foreground">
                            {reason.title}
                          </span>
                          {' — '}
                          {reason.body}
                        </p>
                      </div>
                    ))}
                  </div>
                </div>
              </Reveal>

              <Reveal delay={120}>
                <div className="aspect-[4/3] overflow-hidden bg-muted">
                  <img
                    src="/images/about/lisa-work.jpg"
                    alt="Lisa Penfield working with clients in Culebra"
                    className="h-full w-full object-cover"
                  />
                </div>
              </Reveal>
            </div>
          </div>
        </section>

        {/* Stats */}
        <section className="px-6 py-16 md:px-12 md:py-20">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid grid-cols-2 gap-y-10 md:grid-cols-4">
              {STATS.map((stat, i) => (
                <Reveal key={stat.v} delay={i * 80}>
                  <div className="text-center md:border-r md:border-border last:md:border-r-0">
                    <p className="font-serif text-4xl font-light text-accent md:text-5xl">
                      {stat.k}
                    </p>
                    <p className="mt-3 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
                      {stat.v}
                    </p>
                  </div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* Life on the island */}
        <section className="px-6 pb-20 pt-8 md:px-12 md:pb-28">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <div className="text-center">
                <h2 className="font-serif text-3xl font-light text-foreground md:text-4xl">
                  Life on the island
                </h2>
                <p className="mx-auto mt-4 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                  A quiet collection of business and personal moments that reflect the
                  pace, place, and perspective behind the brand.
                </p>
              </div>
            </Reveal>

            <div className="mt-12 flex gap-2 overflow-x-auto pb-2">
              {LIFE_IMAGES.map((src, i) => (
                <Reveal
                  key={src}
                  delay={i * 50}
                >
                  <div className="h-[220px] w-[180px] shrink-0 overflow-hidden bg-muted md:h-[260px] md:w-[220px]">
                    <img
                      src={src}
                      alt={`Life on Culebra ${i + 1}`}
                      className="h-full w-full object-cover"
                    />
                  </div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* CTA */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
          <div className="mx-auto flex max-w-[1600px] flex-col items-start gap-8">
            <Reveal>
              <h2 className="max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                We would be glad to know what you are looking for.
              </h2>
            </Reveal>

            <Reveal delay={120}>
              <Link
                href="/contact"
                className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
              >
                Start a conversation
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

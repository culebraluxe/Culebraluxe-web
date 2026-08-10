import type { Metadata } from 'next'
import Link from 'next/link'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'

export const metadata: Metadata = {
  title: 'About Us — CulebraLuxe',
  description:
    'CulebraLuxe is a boutique brokerage devoted to a single island — working with few clients, few homes, and an uncommon amount of care.',
}

const VALUES = [
  {
    title: 'Fit over volume',
    body: 'We measure success not in transactions but in fit — pairing the right stewards with the right homes.',
  },
  {
    title: 'Local, truly',
    body: 'Founded by island residents, we know Culebra beyond its coordinates — the trade winds, the tide charts, the families who shaped it.',
  },
  {
    title: 'Quiet stewardship',
    body: 'We protect the character that makes this place rare, advising with discretion and patience at every turn.',
  },
]

const STATS = [
  { k: '14', v: 'Years on island' },
  { k: '1', v: 'Island, entirely' },
  { k: '40+', v: 'Homes stewarded' },
  { k: '100%', v: 'By referral' },
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
          image="/images/culture.png"
          imageAlt="The turquoise water and white sand of Flamenco Beach, Culebra"
        />

        {/* Story */}
        <section className="px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <h2 className="max-w-5xl text-balance font-serif text-3xl font-light leading-[1.2] text-foreground md:text-5xl md:leading-[1.18]">
                We work with few clients, few homes, and an uncommon amount of care.
              </h2>
            </Reveal>
            <div className="mt-20 grid gap-14 border-t border-border pt-16 md:mt-28 md:grid-cols-2 md:gap-24">
              <Reveal delay={120}>
                <p className="max-w-md text-sm font-light leading-relaxed text-muted-foreground">
                  Founded by island residents, we know Culebra beyond its coordinates — the
                  trade winds, the tide charts, the families who have shaped it for
                  generations. Our practice grew not from ambition to scale, but from a
                  desire to do a small thing exceptionally well.
                </p>
              </Reveal>
              <Reveal delay={220}>
                <p className="max-w-md text-sm font-light leading-relaxed text-muted-foreground">
                  We measure success not in volume but in fit — pairing the right stewards
                  with the right homes, and protecting the character that makes this place
                  rare. Nearly all of our work arrives by quiet referral, and we intend to
                  keep it that way.
                </p>
              </Reveal>
            </div>
          </div>
        </section>

        {/* Values */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <p className="mb-16 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50 md:mb-20">
                What we value
              </p>
            </Reveal>
            <div className="grid gap-14 md:grid-cols-3 md:gap-16">
              {VALUES.map((value, i) => (
                <Reveal key={value.title} delay={i * 120}>
                  <h3 className="font-serif text-2xl font-light leading-snug">{value.title}</h3>
                  <p className="mt-4 max-w-sm text-sm font-light leading-relaxed text-primary-foreground/75">
                    {value.body}
                  </p>
                </Reveal>
              ))}
            </div>
            <div className="mt-20 grid grid-cols-2 gap-10 border-t border-primary-foreground/10 pt-14 md:mt-28 md:grid-cols-4">
              {STATS.map((stat, i) => (
                <Reveal key={stat.v} delay={i * 90}>
                  <p className="font-serif text-4xl font-light">{stat.k}</p>
                  <p className="mt-2 text-xs font-light uppercase tracking-[0.2em] text-primary-foreground/60">
                    {stat.v}
                  </p>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* CTA */}
        <section className="px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto flex max-w-[1600px] flex-col items-start gap-8">
            <Reveal>
              <h2 className="max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                We would be glad to know what you are looking for.
              </h2>
            </Reveal>
            <Reveal delay={120}>
              <Link
                href="/contact"
                className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em] text-foreground"
              >
                Start a conversation
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

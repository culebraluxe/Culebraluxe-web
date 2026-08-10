import type { Metadata } from 'next'
import Link from 'next/link'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'

export const metadata: Metadata = {
  title: 'Island Guide — CulebraLuxe',
  description:
    'A quiet guide to life on Culebra — its beaches, marine reserves, and the unhurried rhythm that rewards those who arrive attentively.',
}

const SECTIONS = [
  {
    eyebrow: 'The Beaches',
    title: 'Flamenco, and the coves beyond it.',
    body: 'Flamenco Beach is consistently ranked among the world\u2019s finest — a white-sand crescent over impossibly clear water. Beyond it lie quieter coves reached on foot or by boat: Zoni, Tamarindo, Carlos Rosario, each with its own light and mood.',
  },
  {
    eyebrow: 'The Water',
    title: 'A protected marine reserve.',
    body: 'Reefs and cays surround the island within a protected marine reserve. Snorkeling begins at the shoreline; sea turtles graze the grass beds, and the water stays reef-clear through the middle of the day.',
  },
  {
    eyebrow: 'Getting Here',
    title: 'Thirty minutes from the mainland.',
    body: 'A short flight or ferry connects Culebra to mainland Puerto Rico. Golf carts outnumber cars; there are no traffic lights and no high-rises — only fishing boats at dawn and evenings measured in shades of gold.',
  },
  {
    eyebrow: 'The Rhythm',
    title: 'A slower pace, kept intentionally intact.',
    body: 'Culebra rewards those who choose to arrive quietly and stay attentively. Days are unhurried; the community is small and protective of the character that makes the island rare.',
  },
]

const FACTS = [
  { k: 'Flamenco', v: 'Among the world\u2019s finest beaches.' },
  { k: 'Marine Reserve', v: 'Protected reefs and cays surround the island.' },
  { k: '30 Minutes', v: 'A short flight or ferry from mainland Puerto Rico.' },
]

export default function GuidePage() {
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

        {/* Quick facts */}
        <section className="px-6 pt-24 md:px-12 md:pt-32">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid gap-10 border-b border-border pb-16 sm:grid-cols-3">
              {FACTS.map((fact, i) => (
                <Reveal key={fact.k} delay={i * 90}>
                  <p className="font-serif text-xl font-light text-foreground">{fact.k}</p>
                  <p className="mt-2 text-sm font-light leading-relaxed text-muted-foreground">
                    {fact.v}
                  </p>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* Editorial sections */}
        <section className="px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid gap-16 md:gap-20">
              {SECTIONS.map((section, i) => (
                <Reveal key={section.eyebrow} delay={i * 80}>
                  <div className="grid gap-6 md:grid-cols-12 md:gap-16">
                    <p className="text-xs font-light uppercase tracking-[0.28em] text-accent md:col-span-3">
                      {section.eyebrow}
                    </p>
                    <div className="md:col-span-9">
                      <h2 className="max-w-2xl text-balance font-serif text-2xl font-light leading-[1.25] text-foreground md:text-3xl">
                        {section.title}
                      </h2>
                      <p className="mt-5 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                        {section.body}
                      </p>
                    </div>
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

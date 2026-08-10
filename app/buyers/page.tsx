import type { Metadata } from 'next'
import Link from 'next/link'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { HomeProperties } from '@/components/home-properties'

export const metadata: Metadata = {
  title: 'For Buyers — CulebraLuxe',
  description:
    'A considered, discreet path to owning an architectural estate or beachfront residence on the island of Culebra, Puerto Rico.',
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

export default function BuyersPage() {
  return (
    <>
      <SiteHeader />
      <main>
        <PageHero
          eyebrow="For Buyers"
          title="A considered path to your place on the island."
          intro="We represent a small number of buyers each year, guiding every stage with discretion — from private viewings and title diligence to residency, architecture, and the quiet logistics of island life."
          image="/images/hero-villa.png"
          imageAlt="A modern luxury villa perched above the Culebra coastline at dusk"
        />

        {/* Process */}
        <section className="px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <p className="mb-16 text-xs font-light uppercase tracking-[0.34em] text-accent md:mb-24">
                The Process
              </p>
            </Reveal>
            <div className="grid gap-14 md:grid-cols-2 md:gap-x-24 md:gap-y-20">
              {STEPS.map((step, i) => (
                <Reveal key={step.n} delay={i * 100}>
                  <div className="flex gap-8 border-t border-border pt-8">
                    <span className="font-serif text-2xl font-light text-accent">{step.n}</span>
                    <div>
                      <h2 className="font-serif text-2xl font-light leading-snug text-foreground">
                        {step.title}
                      </h2>
                      <p className="mt-4 max-w-md text-sm font-light leading-relaxed text-muted-foreground">
                        {step.body}
                      </p>
                    </div>
                  </div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* Current listings — pulls live from Sanity, each card links to its detail page */}
        <HomeProperties
          eyebrow="Current Listings"
          title="Homes currently on the island."
          intro="A selection of the residences and land we are representing now. Select any property to explore its full details, gallery, and location."
          limit={6}
          cta={null}
        />

        {/* Services + CTA */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
          <div className="mx-auto grid max-w-[1600px] gap-14 md:grid-cols-2 md:gap-24">
            <Reveal>
              <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
                How we help
              </p>
              <h2 className="text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                Every detail, quietly handled.
              </h2>
            </Reveal>
            <Reveal delay={120} className="flex flex-col gap-10">
              <ul className="flex flex-col divide-y divide-primary-foreground/10 border-y border-primary-foreground/10">
                {SERVICES.map((item) => (
                  <li
                    key={item}
                    className="py-5 text-sm font-light tracking-wide text-primary-foreground/85"
                  >
                    {item}
                  </li>
                ))}
              </ul>
              <Link
                href="/contact"
                className="group inline-flex items-center gap-3 self-start text-xs font-light uppercase tracking-[0.24em]"
              >
                Begin a search
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

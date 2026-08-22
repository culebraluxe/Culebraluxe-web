import type { Metadata } from 'next'
import Link from 'next/link'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { FaqAccordion } from '@/components/faq-accordion'
import { getMarketingContent } from '@/db/marketing-content'
import { buildFaqPageContent } from '@/lib/marketing-content'

export const metadata: Metadata = {
  title: 'Frequently Asked Questions — CulebraLuxe',
  description:
    'Common questions about buying, owning, and living on the island of Culebra, Puerto Rico.',
}

export const dynamic = 'force-dynamic'

export default async function FaqPage() {
  const page = buildFaqPageContent(await getMarketingContent())

  return (
    <>
      <SiteHeader />
      <main>
        {page.hero ? (
          <PageHero
            eyebrow={page.hero.eyebrow ?? ''}
            title={page.hero.title ?? ''}
            intro={page.hero.body ?? undefined}
            image={page.hero.imagePath ?? '/images/hero-villa.png'}
            imageAlt={page.hero.imageAlt ?? 'A luxury villa overlooking the Culebra coastline'}
          />
        ) : null}

        <section className="px-6 py-24 md:px-12 md:py-32">
          <FaqAccordion items={page.entries} />

          <div className="mx-auto mt-20 flex max-w-3xl flex-col items-start gap-6 border-t border-border pt-12">
            <Reveal>
              <p className="text-pretty font-serif text-2xl font-light leading-snug text-foreground">
                {page.ctaHeading}
              </p>
            </Reveal>
            <Reveal delay={100}>
              <Link
                href={page.ctaHref ?? '/contact'}
                className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em] text-foreground"
              >
                {page.ctaLabel}
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

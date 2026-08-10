import type { Metadata } from 'next'
import Link from 'next/link'
import { SiteHeader } from '@/components/site-header'
import { SiteFooter } from '@/components/site-footer'
import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { FaqAccordion } from '@/components/faq-accordion'

export const metadata: Metadata = {
  title: 'Frequently Asked Questions — CulebraLuxe',
  description:
    'Common questions about buying, owning, and living on the island of Culebra, Puerto Rico.',
}

const FAQS = [
  {
    q: 'Can anyone buy property in Culebra?',
    a: 'Yes. Culebra is part of Puerto Rico, a United States territory, so U.S. citizens buy here without restriction and the process will feel familiar. Buyers from elsewhere are welcome as well; we guide every client through the particulars.',
  },
  {
    q: 'How do I actually get to the island?',
    a: 'Culebra sits about 30 minutes from mainland Puerto Rico by a short flight from San Juan or Ceiba, or by ferry from Ceiba. Once here, most residents move about by golf cart — there are no traffic lights on the island.',
  },
  {
    q: 'Are many of your homes off-market?',
    a: 'Often, yes. Many of Culebra\u2019s finest residences never reach a public listing. We arrange discreet viewings of off-market homes held within our private network, matched to what each buyer is genuinely seeking.',
  },
  {
    q: 'What are the ongoing costs of owning here?',
    a: 'Beyond the purchase, owners should plan for property taxes, insurance, utilities, and — for many homes — management or caretaking while away. We provide clear estimates for any specific property and introductions to trusted local services.',
  },
  {
    q: 'Can you help with residency or relocation?',
    a: 'We support clients through the practical logistics of island life, from residency questions to introductions for architects, builders, and property managers. Our involvement does not end at closing.',
  },
  {
    q: 'How do you work with sellers?',
    a: 'We take on a small number of listings and give each an editorial presentation — considered photography, measured storytelling, and introductions to a private circle of international buyers. It begins with a confidential valuation.',
  },
]

export default function FaqPage() {
  return (
    <>
      <SiteHeader />
      <main>
        <PageHero
          eyebrow="Frequently Asked"
          title="Questions, quietly answered."
          intro="A few of the things buyers and sellers most often ask us about life and property on Culebra."
          image="/images/hero-villa.png"
          imageAlt="A luxury villa overlooking the Culebra coastline"
        />

        <section className="px-6 py-24 md:px-12 md:py-32">
          <FaqAccordion items={FAQS} />

          <div className="mx-auto mt-20 flex max-w-3xl flex-col items-start gap-6 border-t border-border pt-12">
            <Reveal>
              <p className="text-pretty font-serif text-2xl font-light leading-snug text-foreground">
                Have a question we have not answered here?
              </p>
            </Reveal>
            <Reveal delay={100}>
              <Link
                href="/contact"
                className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em] text-foreground"
              >
                Ask us directly
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

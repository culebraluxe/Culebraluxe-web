import type { Metadata } from 'next'
import Image from 'next/image'
import Link from 'next/link'

import {
  CheckCircle2,
  ClipboardCheck,
  Compass,
  FileSearch,
  Handshake,
  MapPinned,
  MessageCircle,
  Network,
  UserRound,
} from 'lucide-react'

import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { SiteFooter } from '@/components/site-footer'
import { SiteHeader } from '@/components/site-header'

export const metadata: Metadata = {
  title: 'Real Estate Services — CulebraLuxe',
  description:
    'Thoughtful real estate advisory, research, coordination, and property services for owners, buyers, and clients on Culebra.',
}

const SERVICES = [
  {
    number: '01',
    title: 'Market Analysis / CMA',
    body: 'Comprehensive market data and local insight to help you understand current value and position with confidence.',
    cta: 'Request analysis',
    href: '/contact?service=market-analysis',
    image: '/images/services/service-01-market-analysis-cma.jpg',
  },
  {
    number: '02',
    title: 'Property Evaluation',
    body: 'A considered evaluation of your home or land based on property characteristics, location, and current market conditions.',
    cta: 'Request evaluation',
    href: '/contact?service=property-evaluation',
    image: '/images/services/service-02-property-evaluation.jpg',
  },
  {
    number: '03',
    title: 'Comparable Research',
    body: 'Detailed comparable-property research to support informed decisions when buying, selling, or evaluating an opportunity.',
    cta: 'Request comparables',
    href: '/contact?service=comparable-research',
    image: '/images/services/service-03-comparable-research.jpg',
  },
  {
    number: '04',
    title: 'Land Survey Coordination',
    body: 'Coordination with trusted local professionals for surveys, boundary work, and related property documentation.',
    cta: 'Request survey',
    href: '/contact?service=land-survey',
    image: '/images/services/service-04-land-survey-coordination.jpg',
  },
  {
    number: '05',
    title: 'Appraisal Coordination',
    body: 'Assistance arranging professional appraisal services for lending, estate planning, investment, or personal decision-making.',
    cta: 'Request appraisal',
    href: '/contact?service=appraisal',
    image: '/images/services/service-05-appraisal-coordination.jpg',
  },
  {
    number: '06',
    title: 'Deed & Title Research',
    body: 'Coordination of title history, deed research, lien checks, and document retrieval with the appropriate local professionals.',
    cta: 'Request research',
    href: '/contact?service=title-research',
    image: '/images/services/service-06-deed-title-research.jpg',
  },
  {
    number: '07',
    title: 'Real Estate Consultation',
    body: 'Personalized guidance for property ownership, purchases, sales, investment questions, and long-range planning on Culebra.',
    cta: 'Book consultation',
    href: '/contact?service=consultation',
    image: '/images/services/service-07-real-estate-consultation.jpg',
  },
  {
    number: '08',
    title: 'Property Marketing Services',
    body: 'Discreet, elevated property presentation and marketing support designed around the property, audience, and objective.',
    cta: 'Discuss marketing',
    href: '/contact?service=property-marketing',
    image: '/images/services/service-08-property-marketing-services.jpg',
  },
]

const PROCESS = [
  {
    number: '1',
    title: 'Share your needs',
    body: 'Tell us about the property, your objectives, and the support you are looking for.',
    icon: MessageCircle,
  },
  {
    number: '2',
    title: 'We review & coordinate',
    body: 'We research the situation, connect the right professionals, and organize the details.',
    icon: MapPinned,
  },
  {
    number: '3',
    title: 'Clear next steps',
    body: 'You receive thoughtful guidance, timely updates, and a clear path forward.',
    icon: ClipboardCheck,
  },
]

const REASONS = [
  {
    title: 'Island-specific knowledge',
    body: 'Deep understanding of Culebra’s properties, neighborhoods, infrastructure, market, and way of life.',
    icon: Compass,
  },
  {
    title: 'Personally handled',
    body: 'Thoughtful, attentive service with direct involvement rather than a high-volume handoff model.',
    icon: UserRound,
  },
  {
    title: 'Trusted local network',
    body: 'Established relationships with surveyors, attorneys, appraisers, contractors, and other island professionals.',
    icon: Network,
  },
  {
    title: 'Boutique service',
    body: 'Selective client relationships, careful coordination, and discreet high-touch support.',
    icon: Handshake,
  },
]

export default function ServicesPage() {
  return (
    <>
      <SiteHeader />

      <main>
        {/* HERO */}
        <PageHero
          eyebrow="Services"
          title="Real estate services, quietly handled."
          intro="Thoughtful advisory, research, coordination, and property support for owners, buyers, and clients across Culebra."
          image="/images/coastline.png"
          imageAlt="Aerial view of Culebra coastline and turquoise Caribbean water"
        />

        {/* INTRO */}
        <section className="border-b border-border bg-[#f8f6f1] px-6 py-20 md:px-12 md:py-24">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <div className="mx-auto max-w-3xl text-center">
                <h2 className="font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                  More than transactions.
                  <br />
                  Thoughtful support at every step.
                </h2>

                <div className="mx-auto mt-6 h-px w-12 bg-accent" />

                <p className="mx-auto mt-7 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                  From valuations and research to coordination and marketing,
                  our services are designed to simplify decisions, connect the
                  right expertise, and protect your interests on Culebra.
                </p>

                <p className="mt-7 text-[10px] font-medium uppercase tracking-[0.24em] text-accent">
                  Local knowledge · Thoughtful coordination · Exceptional discretion
                </p>
              </div>
            </Reveal>
          </div>
        </section>

        {/* SERVICES */}
        <section className="border-b border-border bg-[#f3efe8] px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <div className="mb-12 md:mb-16">
                <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                  What we can help with
                </p>

                <h2 className="mt-4 max-w-2xl font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                  Practical expertise around island property.
                </h2>
              </div>
            </Reveal>

            <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
              {SERVICES.map((service, i) => (
                <Reveal key={service.title} delay={i * 60}>
                  <article className="group flex h-full flex-col overflow-hidden border border-border bg-[#fbfaf7] transition-transform duration-500 hover:-translate-y-1">
                    {/* IMAGE PLACEHOLDER */}
                <div className="relative aspect-[4/2.7] overflow-hidden bg-muted">
  <Image
    src={service.image}
    alt={service.title}
    fill
    sizes="(min-width: 1024px) 25vw, (min-width: 640px) 50vw, 100vw"
    className="object-cover transition-transform duration-700 group-hover:scale-[1.03]"
  />
</div>

                    <div className="flex flex-1 flex-col px-6 pb-7 pt-6">
                      <p className="text-[10px] font-light uppercase tracking-[0.22em] text-accent">
                        {service.number}
                      </p>

                      <h3 className="mt-3 font-serif text-xl font-light leading-tight text-foreground">
                        {service.title}
                      </h3>

                      <p className="mt-4 flex-1 text-sm font-light leading-relaxed text-muted-foreground">
                        {service.body}
                      </p>

                      <Link
                        href={service.href}
                        className="group/link mt-7 inline-flex items-center gap-3 text-[10px] font-medium uppercase tracking-[0.18em] text-accent"
                      >
                        {service.cta}

                        <span className="inline-block h-px w-6 bg-accent transition-all duration-500 group-hover/link:w-10" />
                      </Link>
                    </div>
                  </article>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* HOW IT WORKS */}
        <section className="border-b border-border bg-[#efebe3] px-6 py-20 md:px-12 md:py-24">
          <div className="mx-auto max-w-[1600px]">
            <Reveal>
              <div className="text-center">
                <h2 className="font-serif text-3xl font-light text-foreground md:text-4xl">
                  How it works
                </h2>

                <div className="mx-auto mt-5 h-px w-12 bg-accent" />
              </div>
            </Reveal>

            <div className="mx-auto mt-14 grid max-w-6xl gap-12 md:grid-cols-3 md:gap-0">
              {PROCESS.map((step, i) => {
                const Icon = step.icon

                return (
                  <Reveal key={step.title} delay={i * 100}>
                    <div className="h-full px-6 text-center md:border-r md:border-border md:px-14 last:md:border-r-0">
                      <div className="mx-auto flex h-14 w-14 items-center justify-center text-accent">
                        <Icon
                          className="h-11 w-11"
                          strokeWidth={1.15}
                        />
                      </div>

                      <div className="mt-7 flex items-baseline justify-center gap-4">
                        <span className="font-serif text-3xl font-light text-accent">
                          {step.number}
                        </span>

                        <h3 className="font-serif text-xl font-light text-foreground">
                          {step.title}
                        </h3>
                      </div>

                      <p className="mx-auto mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                        {step.body}
                      </p>
                    </div>
                  </Reveal>
                )
              })}
            </div>
          </div>
        </section>

        {/* WHY CLIENTS */}
        <section className="border-b border-border bg-[#f8f6f1]">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid items-stretch md:grid-cols-2">
              <Reveal>
                <div className="relative min-h-[420px] overflow-hidden bg-muted md:min-h-[620px]">
                  <Image
                    src="/images/hero-villa.png"
                    alt="Culebra property overlooking the Caribbean"
                    fill
                    sizes="(min-width: 768px) 50vw, 100vw"
                    className="object-cover"
                  />
                </div>
              </Reveal>

              <Reveal delay={120}>
                <div className="flex h-full flex-col justify-center px-6 py-16 md:px-14 md:py-20 lg:px-20">
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    Why CulebraLuxe
                  </p>

                  <h2 className="mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                    Why clients come to CulebraLuxe
                  </h2>

                  <div className="mt-9 space-y-7">
                    {REASONS.map((reason) => {
                      const Icon = reason.icon

                      return (
                        <div
                          key={reason.title}
                          className="flex gap-5"
                        >
                          <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-accent/60 text-accent">
                            <Icon
                              className="h-4 w-4"
                              strokeWidth={1.3}
                            />
                          </div>

                          <div>
                            <h3 className="text-sm font-medium text-foreground">
                              {reason.title}
                            </h3>

                            <p className="mt-1.5 max-w-lg text-sm font-light leading-relaxed text-muted-foreground">
                              {reason.body}
                            </p>
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              </Reveal>
            </div>
          </div>
        </section>

        {/* QUIET DISTINCTION */}
        <section className="border-b border-border bg-[#f2ede5] px-6 py-16 md:px-12 md:py-20">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid gap-10 md:grid-cols-3 md:gap-0">
              <Reveal>
                <div className="px-5 text-center md:border-r md:border-border md:px-12">
                  <FileSearch
                    className="mx-auto h-8 w-8 text-accent"
                    strokeWidth={1.2}
                  />

                  <p className="mt-5 text-xs font-medium uppercase tracking-[0.16em] text-foreground">
                    Research before action
                  </p>

                  <p className="mx-auto mt-3 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                    Decisions begin with understanding the property, context,
                    documentation, and objective.
                  </p>
                </div>
              </Reveal>

              <Reveal delay={100}>
                <div className="px-5 text-center md:border-r md:border-border md:px-12">
                  <Handshake
                    className="mx-auto h-8 w-8 text-accent"
                    strokeWidth={1.2}
                  />

                  <p className="mt-5 text-xs font-medium uppercase tracking-[0.16em] text-foreground">
                    The right people
                  </p>

                  <p className="mx-auto mt-3 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                    We help connect each need with appropriate local expertise
                    rather than treating every request the same.
                  </p>
                </div>
              </Reveal>

              <Reveal delay={200}>
                <div className="px-5 text-center md:px-12">
                  <CheckCircle2
                    className="mx-auto h-8 w-8 text-accent"
                    strokeWidth={1.2}
                  />

                  <p className="mt-5 text-xs font-medium uppercase tracking-[0.16em] text-foreground">
                    Follow-through
                  </p>

                  <p className="mx-auto mt-3 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                    Thoughtful coordination and clear communication keep small
                    details from becoming large problems.
                  </p>
                </div>
              </Reveal>
            </div>
          </div>
        </section>

        {/* CTA */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px] text-center">
            <Reveal>
              <p className="text-xs font-light uppercase tracking-[0.28em] text-primary-foreground/60">
                Culebra · Puerto Rico
              </p>

              <h2 className="mx-auto mt-5 max-w-3xl font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                Let&apos;s begin a quiet conversation.
              </h2>

              <p className="mx-auto mt-5 max-w-xl text-sm font-light leading-relaxed text-primary-foreground/70">
                Tell us what you need. We&apos;ll help determine the right next
                step and whether CulebraLuxe can help.
              </p>
            </Reveal>

            <Reveal delay={120}>
              <Link
                href="/contact"
                className="mt-10 inline-flex border border-primary-foreground/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] transition-colors hover:bg-primary-foreground hover:text-primary"
              >
                Start a conversation
              </Link>
            </Reveal>
          </div>
        </section>
      </main>

      <SiteFooter />
    </>
  )
}
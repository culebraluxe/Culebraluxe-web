import type { Metadata } from 'next'
import Image from 'next/image'
import Link from 'next/link'

import {
  BarChart3,
  Camera,
  CheckCircle2,
  ClipboardCheck,
  Compass,
  FileText,
  Flag,
  Gem,
  Handshake,
  Home,
  KeyRound,
  Mail,
  MapPin,
  Megaphone,
  Network,
  PenLine,
  Search,
  Send,
  Target,
  UserRound,
  Users,
  Video,
} from 'lucide-react'

import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { SiteFooter } from '@/components/site-footer'
import { SiteHeader } from '@/components/site-header'


export const metadata: Metadata = {
  title: 'For Sellers — CulebraLuxe',
  description:
    'A considered approach to selling property on Culebra — local market intelligence, individual positioning, exceptional presentation, and personal representation from first conversation through closing.',
}


const WHY_US = [
  {
    title: 'Local intelligence',
    body: 'Deep knowledge of properties, places, and local conditions that do not appear neatly in a database.',
    icon: Compass,
  },
  {
    title: 'Individual attention',
    body: 'Every property receives its own positioning, strategy, presentation, and path to market.',
    icon: UserRound,
  },
  {
    title: 'Selective representation',
    body: 'We maintain a limited portfolio so each listing receives meaningful focus and care.',
    icon: Gem,
  },
]


const PROCESS = [
  {
    title: 'Understand',
    body: 'The property, circumstances, and objectives.',
    icon: Search,
  },
  {
    title: 'Position',
    body: 'Market analysis and pricing strategy.',
    icon: Target,
  },
  {
    title: 'Prepare',
    body: 'Property preparation and media production.',
    icon: Camera,
  },
  {
    title: 'Launch',
    body: 'Market introduction and targeted exposure.',
    icon: Send,
  },
  {
    title: 'Represent',
    body: 'Showings, offers, and skilled negotiation.',
    icon: Users,
  },
  {
    title: 'Close',
    body: 'Contract-to-closing coordination and follow-through.',
    icon: CheckCircle2,
  },
]


const MARKET_LEFT = [
  {
    title: 'Property',
    body: 'Home, improvements, land, views, condition.',
    icon: Home,
  },
  {
    title: 'Place',
    body: 'Micro-location, access, infrastructure, island context.',
    icon: MapPin,
  },
  {
    title: 'Market',
    body: 'Comparable sales, competition, supply, and current conditions.',
    icon: BarChart3,
  },
]


const MARKET_RIGHT = [
  {
    title: 'Buyer',
    body: 'Likely buyer pool, motivations, ability, and timing.',
    icon: UserRound,
  },
  {
    title: 'Objectives',
    body: 'Your goals, timing, flexibility, and desired outcome.',
    icon: Flag,
  },
]


const PRESENTATION = [
  {
    title: 'Professional photography',
    icon: Camera,
  },
  {
    title: 'Video & walkthroughs',
    icon: Video,
  },
  {
    title: 'Staging & presentation guidance',
    icon: Home,
  },
  {
    title: 'Editorial property story',
    icon: FileText,
  },
  {
    title: 'Digital property marketing',
    icon: Megaphone,
  },
  {
    title: 'Print & marketing materials',
    icon: ClipboardCheck,
  },
]


const DISTRIBUTION = [
  {
    title: 'CulebraLuxe buyer relationships',
    icon: Users,
  },
  {
    title: 'Direct qualified-buyer outreach',
    icon: UserRound,
  },
  {
    title: 'Targeted email campaigns',
    icon: Mail,
  },
  {
    title: 'Puerto Rico listing channels & MLS',
    icon: Network,
  },
  {
    title: 'Major real estate platforms',
    icon: Megaphone,
  },
  {
    title: 'Broker network & referrals',
    icon: Handshake,
  },
]


const REPRESENTATION = [
  {
    title: 'Private showings',
    body: 'Personally presenting the property to qualified buyers.',
    icon: Users,
  },
  {
    title: 'Offers & negotiation',
    body: 'Evaluating offers and negotiating terms that align with your goals.',
    icon: FileText,
  },
  {
    title: 'Contract progression',
    body: 'Moving from accepted offer into the appropriate purchase-and-sale process.',
    icon: PenLine,
  },
  {
    title: 'Due diligence coordination',
    body: 'Survey, appraisal, inspections, financing, title, and other diligence items.',
    icon: ClipboardCheck,
  },
  {
    title: 'Closing coordination',
    body: 'Coordinating with attorneys, title professionals, and all parties.',
    icon: Handshake,
  },
  {
    title: 'Successful close',
    body: 'Following through until the transaction is complete.',
    icon: KeyRound,
  },
]


export default function SellersPage() {
  return (
    <>
      <SiteHeader />

      <main>
        <PageHero
          eyebrow="Selling on Culebra"
          title="Presented to the few who truly belong here."
          intro="Extraordinary properties deserve more than exposure — they deserve understanding, strategy, and representation."
          image="/images/coastline.png"
          imageAlt="Culebra coastline and homes overlooking the Caribbean"
        />


        {/* 01 — WHY CULEBRALUXE */}
        <section className="border-b border-border bg-[#f6f3ed] px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid gap-14 md:grid-cols-12 md:gap-16">

              <div className="md:col-span-3">
                <Reveal>
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    01
                  </p>

                  <h2 className="mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                    Why CulebraLuxe
                  </h2>

                  <p className="mt-6 text-sm font-medium leading-relaxed text-foreground">
                    Selling on Culebra is different.
                  </p>

                  <p className="mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                    This is not a conventional real estate market. Inventory is limited,
                    data is fragmented, and every property is unique. Value is shaped by
                    factors an algorithm will never see.
                  </p>
                </Reveal>
              </div>

              <div className="grid gap-10 md:col-span-9 md:grid-cols-3 md:gap-0">
                {WHY_US.map((item, i) => {
                  const Icon = item.icon

                  return (
                    <Reveal key={item.title} delay={i * 100}>
                      <div className="h-full md:border-l md:border-border md:px-10">
                        <Icon
                          className="h-10 w-10 text-accent"
                          strokeWidth={1.25}
                        />

                        <h3 className="mt-6 text-sm font-medium uppercase tracking-[0.15em] text-foreground">
                          {item.title}
                        </h3>

                        <p className="mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                          {item.body}
                        </p>
                      </div>
                    </Reveal>
                  )
                })}
              </div>

            </div>
          </div>
        </section>


        {/* 02 — PROCESS */}
        <section className="border-b border-border bg-[#efebe3] px-6 py-20 md:px-12 md:py-24">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid gap-12 md:grid-cols-12 md:gap-12">

              <div className="md:col-span-2">
                <Reveal>
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    02
                  </p>

                  <h2 className="mt-4 font-serif text-3xl font-light text-foreground">
                    Our Process
                  </h2>

                  <p className="mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                    A considered approach from first conversation to closing.
                  </p>
                </Reveal>
              </div>

              <div className="md:col-span-10">
                <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-6">
                  {PROCESS.map((step, i) => {
                    const Icon = step.icon

                    return (
                      <Reveal key={step.title} delay={i * 60}>
                        <div className="relative h-full bg-[#f8f6f1] px-5 py-7 text-center lg:[clip-path:polygon(0_0,88%_0,100%_50%,88%_100%,0_100%,12%_50%)] lg:px-7">
                          <Icon
                            className="mx-auto h-8 w-8 text-accent"
                            strokeWidth={1.25}
                          />

                          <p className="mt-5 text-[10px] font-medium uppercase tracking-[0.16em] text-foreground">
                            {step.title}
                          </p>

                          <p className="mt-4 text-xs font-light leading-relaxed text-muted-foreground">
                            {step.body}
                          </p>
                        </div>
                      </Reveal>
                    )
                  })}
                </div>
              </div>

            </div>
          </div>
        </section>


        {/* 03 — MARKET POSITIONING */}
        <section className="border-b border-border bg-[#f8f6f1] px-6 py-24 md:px-12 md:py-32">
          <div className="mx-auto max-w-[1600px]">
            <div className="grid gap-14 md:grid-cols-12 md:gap-14">

              <div className="md:col-span-3">
                <Reveal>
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    03
                  </p>

                  <h2 className="mt-4 font-serif text-3xl font-light leading-[1.05] text-foreground md:text-4xl">
                    Market
                    <br />
                    Positioning
                  </h2>

                  <p className="mt-5 text-sm font-medium text-foreground">
                    Position before promotion.
                  </p>

                  <p className="mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                    We analyze five key dimensions to determine how your property should
                    be positioned in today&apos;s Culebra market.
                  </p>
                </Reveal>
              </div>


              <div className="md:col-span-9">
                <Reveal delay={100}>
                  <div className="rounded-sm border border-border bg-[#fcfbf8] px-6 py-10 md:px-10 md:py-12">

                    <div className="grid items-center gap-10 lg:grid-cols-[1fr_auto_1.3fr_auto_1fr] lg:gap-5">


                      {/* LEFT INPUTS */}
                      <div className="space-y-9">
                        {MARKET_LEFT.map((item) => {
                          const Icon = item.icon

                          return (
                            <div key={item.title} className="flex gap-4">
                              <Icon
                                className="mt-1 h-7 w-7 shrink-0 text-accent"
                                strokeWidth={1.25}
                              />

                              <div>
                                <p className="text-[10px] font-medium uppercase tracking-[0.15em] text-foreground">
                                  {item.title}
                                </p>

                                <p className="mt-2 text-xs font-light leading-relaxed text-muted-foreground">
                                  {item.body}
                                </p>
                              </div>
                            </div>
                          )
                        })}
                      </div>


                      {/* LEFT BRACE */}
                      <div className="hidden items-center justify-center lg:flex">
                        <span className="select-none font-serif text-[150px] font-extralight leading-none text-accent/35">
                          {'}'}
                        </span>
                      </div>


                      {/* CENTER ANALYSIS */}
                      <div className="flex min-w-0 flex-col items-center">

                        <div className="flex aspect-square w-full max-w-[310px] flex-col items-center justify-center rounded-full border border-accent/30 bg-[#eee8de] px-10 text-center shadow-[0_8px_30px_rgba(0,0,0,0.025)]">

                          <p className="font-serif text-4xl font-light text-accent">
                            CL
                          </p>

                          <p className="mt-5 text-xs font-medium uppercase tracking-[0.2em] text-foreground">
                            CulebraLuxe
                            <br />
                            Analysis
                          </p>

                          <div className="mt-5 space-y-1 text-xs font-light text-muted-foreground">
                            <p>Local knowledge.</p>
                            <p>Market intelligence.</p>
                            <p>Individual judgment.</p>
                          </div>

                        </div>


                        {/* OUTPUT */}
                        <div className="h-12 w-px bg-accent/30" />

                        <div className="border border-accent/20 bg-[#f3eee6] px-10 py-5 text-center">
                          <p className="text-[10px] font-medium uppercase tracking-[0.16em] text-foreground">
                            Market Position
                          </p>

                          <p className="mt-2 font-serif text-sm font-light text-muted-foreground">
                            Price · Strategy · Timing
                          </p>
                        </div>

                      </div>


                      {/* RIGHT BRACE */}
                      <div className="hidden items-center justify-center lg:flex">
                        <span className="select-none font-serif text-[150px] font-extralight leading-none text-accent/35">
                          {'{'}
                        </span>
                      </div>


                      {/* RIGHT INPUTS */}
                      <div className="space-y-12">
                        {MARKET_RIGHT.map((item) => {
                          const Icon = item.icon

                          return (
                            <div key={item.title} className="flex gap-4">
                              <Icon
                                className="mt-1 h-7 w-7 shrink-0 text-accent"
                                strokeWidth={1.25}
                              />

                              <div>
                                <p className="text-[10px] font-medium uppercase tracking-[0.15em] text-foreground">
                                  {item.title}
                                </p>

                                <p className="mt-2 text-xs font-light leading-relaxed text-muted-foreground">
                                  {item.body}
                                </p>
                              </div>
                            </div>
                          )
                        })}
                      </div>

                    </div>


                    {/* MOBILE EXPLANATION */}
                    <div className="mt-10 border-t border-border pt-6 text-center lg:hidden">
                      <p className="text-[10px] font-light uppercase tracking-[0.18em] text-muted-foreground">
                        Five dimensions inform one considered market position.
                      </p>
                    </div>

                  </div>
                </Reveal>
              </div>

            </div>
          </div>
        </section>


        {/* 04 — PRESENTATION & EXPOSURE */}
        <section className="border-b border-border bg-[#f0ece5] px-6 py-24 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">

            <div className="grid gap-14 md:grid-cols-12 md:gap-14">

              <div className="md:col-span-3">
                <Reveal>
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    04
                  </p>

                  <h2 className="mt-4 font-serif text-3xl font-light leading-[1.05] text-foreground md:text-4xl">
                    Presentation
                    <br />
                    & Exposure
                  </h2>

                  <p className="mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                    The right property deserves more than a listing.
                  </p>
                </Reveal>

                <Reveal delay={100}>
                  <div className="relative mt-10 aspect-[4/3] overflow-hidden">
                    <Image
                      src="/images/hero-villa.png"
                      alt="Culebra property prepared for market presentation"
                      fill
                      sizes="(min-width: 768px) 25vw, 100vw"
                      className="object-cover"
                    />
                  </div>
                </Reveal>
              </div>


              <div className="grid gap-12 md:col-span-9 md:grid-cols-2 md:gap-0">

                <Reveal delay={100}>
                  <div className="h-full border-border bg-[#faf8f4] p-8 md:border-l md:px-12 md:py-10">

                    <p className="text-xs font-medium uppercase tracking-[0.18em] text-foreground">
                      Presentation
                    </p>

                    <div className="mt-8 space-y-6">
                      {PRESENTATION.map((item) => {
                        const Icon = item.icon

                        return (
                          <div key={item.title} className="flex items-center gap-4">
                            <Icon
                              className="h-5 w-5 shrink-0 text-accent"
                              strokeWidth={1.25}
                            />

                            <p className="text-sm font-light text-foreground">
                              {item.title}
                            </p>
                          </div>
                        )
                      })}
                    </div>

                  </div>
                </Reveal>


                <Reveal delay={180}>
                  <div className="h-full border-border bg-[#faf8f4] p-8 md:border-l md:px-12 md:py-10">

                    <p className="text-xs font-medium uppercase tracking-[0.18em] text-foreground">
                      Distribution
                    </p>

                    <div className="mt-8 space-y-6">
                      {DISTRIBUTION.map((item) => {
                        const Icon = item.icon

                        return (
                          <div key={item.title} className="flex items-center gap-4">
                            <Icon
                              className="h-5 w-5 shrink-0 text-accent"
                              strokeWidth={1.25}
                            />

                            <p className="text-sm font-light text-foreground">
                              {item.title}
                            </p>
                          </div>
                        )
                      })}
                    </div>

                  </div>
                </Reveal>

              </div>
            </div>


            <Reveal delay={220}>
              <p className="mt-14 border-t border-border pt-8 text-center font-serif text-lg font-light text-foreground">
                Exposure is not the strategy. Exposure serves the strategy.
              </p>
            </Reveal>

          </div>
        </section>


        {/* 05 — REPRESENTATION */}
        <section className="bg-[#f7f4ef] px-6 py-24 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">

            <div className="grid gap-12 md:grid-cols-12 md:gap-12">

              <div className="md:col-span-2">
                <Reveal>
                  <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                    05
                  </p>

                  <h2 className="mt-4 font-serif text-3xl font-light text-foreground">
                    Representation
                  </h2>

                  <p className="mt-5 text-sm font-light leading-relaxed text-muted-foreground">
                    From first showing to closing.
                  </p>
                </Reveal>
              </div>


              <div className="grid gap-10 sm:grid-cols-2 md:col-span-10 lg:grid-cols-6 lg:gap-0">
                {REPRESENTATION.map((item, i) => {
                  const Icon = item.icon

                  return (
                    <Reveal key={item.title} delay={i * 60}>
                      <div className="h-full text-center lg:border-l lg:border-border lg:px-6">

                        <Icon
                          className="mx-auto h-8 w-8 text-accent"
                          strokeWidth={1.25}
                        />

                        <h3 className="mt-5 text-[10px] font-medium uppercase tracking-[0.15em] text-foreground">
                          {item.title}
                        </h3>

                        <p className="mt-4 text-xs font-light leading-relaxed text-muted-foreground">
                          {item.body}
                        </p>

                      </div>
                    </Reveal>
                  )
                })}
              </div>

            </div>
          </div>
        </section>


        {/* CTA */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px] text-center">

            <Reveal>
              <h2 className="font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                Every property starts with a conversation.
              </h2>

              <p className="mx-auto mt-5 max-w-xl text-sm font-light leading-relaxed text-primary-foreground/70">
                Tell us about your property. We&apos;ll discuss your objectives,
                the market, and whether working together makes sense.
              </p>
            </Reveal>

            <Reveal delay={120}>
              <Link
                href="/contact"
                className="mt-10 inline-flex border border-primary-foreground/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] transition-colors hover:bg-primary-foreground hover:text-primary"
              >
                Discuss your property
              </Link>
            </Reveal>

          </div>
        </section>
      </main>

      <SiteFooter />
    </>
  )
}
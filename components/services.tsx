import Image from 'next/image'
import { Reveal } from '@/components/reveal'

export function Services() {
  return (
    <div className="bg-primary text-primary-foreground">
      {/* Buyers */}
      <section id="buyers" className="border-b border-primary-foreground/10 px-6 py-28 md:px-12 md:py-40">
        <div className="mx-auto grid max-w-[1600px] gap-14 md:grid-cols-2 md:gap-24">
          <Reveal>
            <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
              For Buyers
            </p>
            <h2 className="text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl">
              A considered path to your place on the island.
            </h2>
          </Reveal>
          <Reveal delay={120} className="flex flex-col justify-center gap-10">
            <p className="max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75">
              We represent a small number of buyers each year, guiding every stage with
              discretion — from private viewings and title diligence to residency,
              architecture, and the quiet logistics of island life.
            </p>
            <ul className="flex flex-col divide-y divide-primary-foreground/10 border-y border-primary-foreground/10">
              {[
                'Private, unlisted viewings',
                'Legal, title & closing guidance',
                'Architecture & renovation introductions',
              ].map((item) => (
                <li
                  key={item}
                  className="py-5 text-sm font-light tracking-wide text-primary-foreground/85"
                >
                  {item}
                </li>
              ))}
            </ul>
            <a
              href="#contact"
              className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
            >
              Begin a search
              <span className="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16" />
            </a>
          </Reveal>
        </div>
      </section>

      {/* Sellers */}
      <section id="sellers" className="px-6 py-28 md:px-12 md:py-40">
        <div className="mx-auto grid max-w-[1600px] items-center gap-14 md:grid-cols-2 md:gap-24">
          <Reveal className="order-2 md:order-1">
            <div className="relative aspect-[4/5] w-full overflow-hidden">
              <Image
                src="/images/coastline.png"
                alt="Aerial view of the Culebra coastline with jade and turquoise water"
                fill
                sizes="(min-width: 768px) 50vw, 100vw"
                className="object-cover"
              />
            </div>
          </Reveal>
          <Reveal delay={120} className="order-1 flex flex-col gap-10 md:order-2">
            <div>
              <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
                For Sellers
              </p>
              <h2 className="text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl">
                Presented to the few who truly belong here.
              </h2>
            </div>
            <p className="max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75">
              Your home deserves more than a listing. We craft an editorial presentation —
              considered photography, measured storytelling, and introductions to a private
              network of international buyers who understand Culebra.
            </p>
            <a
              href="#contact"
              className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
            >
              Request a valuation
              <span className="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16" />
            </a>
          </Reveal>
        </div>
      </section>
    </div>
  )
}

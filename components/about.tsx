import { Reveal } from '@/components/reveal'

export function About() {
  return (
    <section id="about" className="px-6 py-28 md:px-12 md:py-40">
      <div className="mx-auto max-w-[1600px]">
        <Reveal>
          <p className="mb-16 text-xs font-light uppercase tracking-[0.34em] text-accent md:mb-24">
            About Us
          </p>
        </Reveal>

        <Reveal delay={80}>
          <h2 className="max-w-5xl text-balance font-serif text-3xl font-light leading-[1.2] text-foreground md:text-5xl md:leading-[1.18]">
            CulebraLuxe is a boutique brokerage devoted to a single island. We work with
            few clients, few homes, and an uncommon amount of care.
          </h2>
        </Reveal>

        <div className="mt-20 grid gap-14 border-t border-border pt-16 md:mt-28 md:grid-cols-3 md:gap-16">
          <Reveal delay={120}>
            <p className="max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
              Founded by island residents, we know Culebra beyond its coordinates — the
              trade winds, the tide charts, the families who have shaped it for
              generations.
            </p>
          </Reveal>
          <Reveal delay={220}>
            <p className="max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
              We measure success not in volume but in fit — pairing the right stewards with
              the right homes, and protecting the character that makes this place rare.
            </p>
          </Reveal>
          <Reveal delay={320} className="flex flex-col justify-between gap-8">
            <div className="grid grid-cols-2 gap-8">
              <div>
                <p className="font-serif text-4xl font-light text-foreground">14</p>
                <p className="mt-2 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                  Years on island
                </p>
              </div>
              <div>
                <p className="font-serif text-4xl font-light text-foreground">1</p>
                <p className="mt-2 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                  Island, entirely
                </p>
              </div>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}

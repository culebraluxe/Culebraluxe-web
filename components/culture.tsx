import Image from 'next/image'
import { Reveal } from '@/components/reveal'

export function Culture() {
  return (
    <section id="culture" className="relative">
      {/* Full-viewport cinematic image */}
      <div className="relative h-[85svh] w-full overflow-hidden">
        <Image
          src="/images/culture.png"
          alt="The white sand crescent and clear turquoise water of Flamenco Beach, Culebra"
          fill
          sizes="100vw"
          className="object-cover"
        />
        <div className="absolute inset-0 bg-gradient-to-t from-black/55 via-black/15 to-black/25" />

        <div className="absolute inset-0 flex flex-col justify-end px-6 pb-20 md:px-12 md:pb-28">
          <div className="mx-auto w-full max-w-[1600px]">
            <Reveal>
              <p className="mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70">
                Island Culture
              </p>
              <h2 className="max-w-3xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
                A slower rhythm, kept intentionally intact.
              </h2>
            </Reveal>
          </div>
        </div>
      </div>

      {/* Editorial body */}
      <div className="px-6 py-24 md:px-12 md:py-32">
        <div className="mx-auto grid max-w-[1600px] gap-14 md:grid-cols-12 md:gap-24">
          <Reveal className="md:col-span-4">
            <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
              Life on Culebra
            </p>
          </Reveal>
          <Reveal delay={120} className="md:col-span-8">
            <p className="max-w-3xl text-balance font-serif text-2xl font-light leading-[1.4] text-foreground md:text-3xl">
              No traffic lights. No high-rises. Fishing boats at dawn, reef-clear water by
              noon, and evenings measured in shades of gold. Culebra rewards those who
              choose to arrive quietly and stay attentively.
            </p>
            <div className="mt-14 grid gap-10 border-t border-border pt-10 sm:grid-cols-3">
              {[
                { k: 'Flamenco', v: 'Consistently ranked among the world\u2019s finest beaches.' },
                { k: 'Marine Reserve', v: 'Protected reefs and cays surround the island.' },
                { k: '30 Minutes', v: 'A short flight or ferry from mainland Puerto Rico.' },
              ].map((stat) => (
                <div key={stat.k}>
                  <p className="font-serif text-xl font-light text-foreground">{stat.k}</p>
                  <p className="mt-2 text-sm font-light leading-relaxed text-muted-foreground">
                    {stat.v}
                  </p>
                </div>
              ))}
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}

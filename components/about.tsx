import { Reveal } from '@/components/reveal'
import { itemsFor, type MarketingContentBlock } from '@/lib/marketing-content'

export function About({ content }: { content: MarketingContentBlock }) {
  const paragraphs = itemsFor(content, 'paragraph')
  const stats = itemsFor(content, 'stat')

  return (
    <section id="about" className="px-6 py-28 md:px-12 md:py-40">
      <div className="mx-auto max-w-[1600px]">
        <Reveal>
          <p className="mb-16 text-xs font-light uppercase tracking-[0.34em] text-accent md:mb-24">
            {content.eyebrow}
          </p>
        </Reveal>

        <Reveal delay={80}>
          <h2 className="max-w-5xl text-balance font-serif text-3xl font-light leading-[1.2] text-foreground md:text-5xl md:leading-[1.18]">
            {content.title}
          </h2>
        </Reveal>

        <div className="mt-20 grid gap-14 border-t border-border pt-16 md:mt-28 md:grid-cols-3 md:gap-16">
          <Reveal delay={120}>
            <p className="max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
              {content.body}
            </p>
          </Reveal>
          <Reveal delay={220}>
            <p className="max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
              {paragraphs[0]?.value}
            </p>
          </Reveal>
          <Reveal delay={320} className="flex flex-col justify-between gap-8">
            <div className="grid grid-cols-2 gap-8">
              {stats.map((stat) => (
                <div key={stat.label}>
                  <p className="font-serif text-4xl font-light text-foreground">{stat.label}</p>
                  <p className="mt-2 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                    {stat.value}
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

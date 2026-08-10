import Image from 'next/image'
import { Reveal } from '@/components/reveal'

export function PageHero({
  eyebrow,
  title,
  intro,
  image,
  imageAlt,
}: {
  eyebrow: string
  title: string
  intro?: string
  image: string
  imageAlt: string
}) {
  return (
    <section className="relative flex min-h-[68svh] items-end overflow-hidden">
      <Image
        src={image || '/placeholder.svg'}
        alt={imageAlt}
        fill
        priority
        sizes="100vw"
        className="object-cover"
      />
      <div className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/25 to-black/40" />

      <div className="relative w-full px-6 pb-16 pt-40 md:px-12 md:pb-24">
        <div className="mx-auto max-w-[1600px]">
          <Reveal>
            <p className="mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70">
              {eyebrow}
            </p>
            <h1 className="max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
              {title}
            </h1>
          </Reveal>
          {intro ? (
            <Reveal delay={120}>
              <p className="mt-8 max-w-2xl text-pretty text-base font-light leading-relaxed text-background/80 md:text-lg">
                {intro}
              </p>
            </Reveal>
          ) : null}
        </div>
      </div>
    </section>
  )
}

import Image from 'next/image'
import { Reveal } from '@/components/reveal'

const PROPERTIES = [
  {
    id: '01',
    name: 'Casa Horizonte',
    location: 'Punta Melones',
    detail: 'Four bedrooms · Infinity edge · 1.2 acres',
    price: 'Price upon request',
    image: '/images/property-01.png',
    alt: 'Minimalist beachfront villa with floor-to-ceiling glass framed by palms',
  },
  {
    id: '02',
    name: 'Villa Salina',
    location: 'Bahía Sardinas',
    detail: 'Three bedrooms · Ocean terrace · Private cove access',
    price: 'Price upon request',
    image: '/images/property-02.png',
    alt: 'Airy villa interior with floor-to-ceiling windows opening to the ocean',
  },
  {
    id: '03',
    name: 'Estancia Poniente',
    location: 'Monte Resaca',
    detail: 'Five bedrooms · Hillside estate · Panoramic sunset views',
    price: 'Price upon request',
    image: '/images/property-03.png',
    alt: 'Modern hillside estate glowing at dusk surrounded by tropical foliage',
  },
]

export function FeaturedProperties() {
  return (
    <section id="properties" className="px-6 py-28 md:px-12 md:py-40">
      <div className="mx-auto max-w-[1600px]">
        <Reveal className="mb-20 md:mb-28">
          <div className="flex flex-col gap-6 border-b border-border pb-10 md:flex-row md:items-end md:justify-between">
            <div>
              <p className="mb-4 text-xs font-light uppercase tracking-[0.34em] text-accent">
                The Collection
              </p>
              <h2 className="max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-6xl">
                Residences chosen for their silence.
              </h2>
            </div>
            <p className="max-w-xs text-pretty text-sm font-light leading-relaxed text-muted-foreground">
              Each estate is selected in person, for its light, its outlook, and its
              relationship to the sea.
            </p>
          </div>
        </Reveal>

        <div className="flex flex-col gap-28 md:gap-40">
          {PROPERTIES.map((property, index) => (
            <Reveal key={property.id}>
              <article
                className={`grid items-center gap-10 md:grid-cols-12 md:gap-16 ${
                  index % 2 === 1 ? 'md:[direction:rtl]' : ''
                }`}
              >
                <div className="md:col-span-8 md:[direction:ltr]">
                  <div className="group relative aspect-[16/10] w-full overflow-hidden">
                    <Image
                      src={property.image || '/placeholder.svg'}
                      alt={property.alt}
                      fill
                      sizes="(min-width: 768px) 66vw, 100vw"
                      className="object-cover transition-transform duration-[1600ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.04]"
                    />
                  </div>
                </div>

                <div className="md:col-span-4 md:[direction:ltr]">
                  <span className="font-serif text-sm font-light text-accent">
                    ({property.id})
                  </span>
                  <h3 className="mt-4 font-serif text-3xl font-light leading-tight text-foreground md:text-4xl">
                    {property.name}
                  </h3>
                  <p className="mt-3 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
                    {property.location}
                  </p>
                  <p className="mt-8 max-w-xs text-sm font-light leading-relaxed text-foreground/80">
                    {property.detail}
                  </p>
                  <div className="mt-8 flex items-center justify-between border-t border-border pt-6">
                    <span className="text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                      {property.price}
                    </span>
                    <a
                      href="#contact"
                      className="group/link inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground"
                    >
                      Enquire
                      <span className="inline-block h-px w-6 bg-foreground transition-all duration-500 group-hover/link:w-10" />
                    </a>
                  </div>
                </div>
              </article>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  )
}

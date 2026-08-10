import Image from 'next/image'
import {notFound} from 'next/navigation'

import {client} from '@/sanity/lib/client'
import {PROPERTY_BY_SLUG_QUERY} from '@/sanity/lib/queries'
import {urlFor} from '@/sanity/lib/image'
import {SiteHeader} from '@/components/site-header'
import {SiteFooter} from '@/components/site-footer'
import {PropertyTabs} from '@/components/property/property-tabs'

export default async function PropertyPage({
  params,
}: {
  params: Promise<{slug: string}>
}) {
  const {slug} = await params

const property = await client.fetch(
  PROPERTY_BY_SLUG_QUERY,
  {slug},
  {cache: 'no-store'}
)

  if (!property) {
    notFound()
  }

  const heroUrl = property.heroImage
    ? urlFor(property.heroImage).width(2000).height(1200).url()
    : null

  return (
     <>
     <SiteHeader />
    <main style={{padding: '48px'}}>
      <p
  style={{
    fontSize: '12px',
    letterSpacing: '0.18em',
    textTransform: 'uppercase',
    marginBottom: '12px',
  }}
>
  Culebra · Puerto Rico
</p>

     <h1
  style={{
    fontFamily: 'var(--font-cormorant)',
    fontSize: 'clamp(32px, 4vw, 52px)',
    fontWeight: 400,
    lineHeight: 0.95,
    margin: 0,
  }}
>
  {property.title}
</h1>

<p
  style={{
    fontSize: '18px',
    marginTop: '18px',
    marginBottom: 0,
  }}
>
  {property.listPrice
    ? `$${property.listPrice.toLocaleString()}`
    : 'Price Upon Request'}
</p>

      {heroUrl && (
        <div
          style={{
            position: 'relative',
            width: '100%',
            height: 'clamp(420px, 60vh, 700px)',
            marginTop: '32px',
          }}
        >
          <Image
            src={heroUrl}
            alt={property.title ?? 'Property'}
            fill
            priority
            style={{objectFit: 'cover'}}
          />
        </div>
      )}

      <div
  style={{
    marginTop: '32px',
    paddingTop: '20px',
    paddingBottom: '20px',
    borderTop: '1px solid rgba(0,0,0,0.18)',
    borderBottom: '1px solid rgba(0,0,0,0.18)',
    fontSize: '13px',
    letterSpacing: '0.12em',
  }}
>
        {property.bedroomsTotal != null && (
          <span>{property.bedroomsTotal} BED</span>
        )}

        {property.bathroomsTotal != null && (
          <span> · {property.bathroomsTotal} BATH</span>
        )}

        {property.livingArea != null && (
          <span> · {property.livingArea.toLocaleString()} SF</span>
        )}

    {property.lotSizeArea != null && (
  <span>
    {' '}
    · {property.lotSizeArea}{' '}
    {property.lotSizeArea === 1 && property.lotSizeUnits === 'Acres'
      ? 'Acre'
      : property.lotSizeUnits ?? ''}
  </span>
)}
      </div>

       <PropertyTabs property={property} />

      {property.shortDescription && (
        <p style={{marginTop: '32px', maxWidth: '700px'}}>
          {property.shortDescription}
        </p>
      )}

     


    </main>
        <SiteFooter />
  </>
  )
}

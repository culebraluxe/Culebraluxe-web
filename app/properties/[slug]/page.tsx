import Image from 'next/image'
import {notFound} from 'next/navigation'

import {client} from '@/sanity/lib/client'
import {PROPERTY_BY_SLUG_QUERY} from '@/sanity/lib/queries'
import {urlFor} from '@/sanity/lib/image'

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
    <main style={{padding: '48px'}}>
      <p>{property.neighborhood ?? 'Culebra'}</p>

      <h1>{property.title}</h1>

      <p>
        {property.listPrice
          ? `$${property.listPrice.toLocaleString()}`
          : 'Price Upon Request'}
      </p>

      {heroUrl && (
        <div
          style={{
            position: 'relative',
            width: '100%',
            height: '65vh',
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

      <div style={{marginTop: '32px'}}>
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

      

      {property.shortDescription && (
        <p style={{marginTop: '32px', maxWidth: '700px'}}>
          {property.shortDescription}
        </p>
      )}
    </main>
  )
}

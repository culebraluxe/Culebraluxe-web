import {notFound} from 'next/navigation'
import {sanityFetch} from '@/sanity/lib/live'
import {PROPERTY_BY_SLUG_QUERY} from '@/sanity/lib/queries'

export default async function PropertyPage({
  params,
}: {
  params: Promise<{slug: string}>
}) {
  const {slug} = await params

  const {data: property} = await sanityFetch({
    query: PROPERTY_BY_SLUG_QUERY,
    params: {slug},
  })

  if (!property) {
    notFound()
  }

  return (
    <main style={{padding: '48px'}}>
      <h1>{property.title}</h1>

      <p>
        {property.listPrice
          ? `$${property.listPrice.toLocaleString()}`
          : 'Price Upon Request'}
      </p>

      <p>Status: {property.standardStatus}</p>

      <p>
        {property.bedroomsTotal ?? '—'} beds ·{' '}
        {property.bathroomsTotal ?? '—'} baths
      </p>

      <pre>{JSON.stringify(property, null, 2)}</pre>
    </main>
  )
}

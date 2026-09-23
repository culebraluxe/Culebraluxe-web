import type { Metadata } from 'next'
import { YewApp } from '@/components/rust-ui/yew-app'
import { getPropertyBySlug } from '@/lib/property-reads'

// ---------------------------------------------------------------------------
// /properties/:slug — YEW OWNS THE DYNAMIC PROPERTY ROUTE.
//
// The Yew router reads the slug from the browser URL and mounts site-property-detail with that
// scope. The reducer owns gallery/lightbox state and the page effect loads the scoped property.
// There is no second React carousel and no string-renderer interaction shim.
// ---------------------------------------------------------------------------

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }): Promise<Metadata> {
  const { slug } = await params
  const result = await getPropertyBySlug(slug)
  if (!result.ok || !result.data) return { title: 'Property — CulebraLuxe' }
  const { property } = result.data
  const location = [property.neighborhood, property.city].filter(Boolean).join(', ')
  return {
    title: `${property.title} — CulebraLuxe`,
    description: property.shortDescription ?? `${property.title}${location ? ` in ${location}` : ''}, presented by CulebraLuxe.`,
  }
}

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}

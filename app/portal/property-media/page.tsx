import { sql } from '@/db/client'

import { PropertyMediaUploader } from '@/components/portal/property-media-uploader'

type PropertyOption = {
  id: string
  name: string
  slug: string | null
  status: string
}

async function getPropertyOptions(): Promise<PropertyOption[]> {
  const rows = await sql`
    SELECT
      id,
      name,
      slug,
      status
    FROM property
    WHERE archived_at IS NULL
    ORDER BY name
  `

  return rows.map((row) => ({
    id: String(row.id),
    name: String(row.name),
    slug: row.slug ? String(row.slug) : null,
    status: String(row.status),
  }))
}

export default async function PropertyMediaPage() {
  const properties = await getPropertyOptions()

  return (
    <main className="min-h-screen bg-[var(--portal-bg)] px-6 py-10 md:px-10">
      <div className="mx-auto max-w-4xl">
        <div className="mb-10">
          <p className="mb-3 text-xs uppercase tracking-[0.24em] text-[var(--portal-blue-gray)]">
            Property Media
          </p>

          <h1 className="font-serif text-4xl font-light text-[var(--portal-navy)]">
            Assign property images
          </h1>

          <p className="mt-4 max-w-2xl text-sm leading-relaxed text-[var(--portal-muted)]">
            Upload an image and attach it directly to a property as either
            hero or gallery media.
          </p>
        </div>

        <PropertyMediaUploader properties={properties} />
      </div>
    </main>
  )
}
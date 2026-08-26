import { Showings } from "@/components/portal/showings"
import { getProperties } from "@/db/properties"
import { getShowings } from "@/db/showings"

export const dynamic = "force-dynamic"

export default async function ShowingsPage() {
  const [showings, propertiesResult] = await Promise.all([
    getShowings(),
    getProperties(),
  ])
  const properties = propertiesResult.ok ? propertiesResult.data : []

  const propertyOptions = properties.map((property) => ({
    id: property.id,
    name: property.name,
    location: property.location,
  }))

  return <Showings showings={showings} properties={propertyOptions} />
}

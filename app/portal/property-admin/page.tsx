import { PropertyAdmin } from "@/components/portal/property-admin"
import { getPropertyAdmin } from "@/db/property-admin"

export const dynamic = "force-dynamic"

export default async function PropertyAdminPage() {
  const rows = await getPropertyAdmin()

  return <PropertyAdmin rows={rows} />
}

import { PropertyAdmin } from "@/components/portal/property-admin"
import { PropertyCreatePanel } from "@/components/portal/write/property-create-panel"
import { getPropertyAdmin } from "@/db/property-admin"

export const dynamic = "force-dynamic"

export default async function PropertyAdminPage() {
  const rows = await getPropertyAdmin()

  return (
    <>
      <PropertyCreatePanel />
      <PropertyAdmin rows={rows} />
    </>
  )
}

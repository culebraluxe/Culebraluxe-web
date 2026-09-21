import { notFound } from "next/navigation"

import { PropertyAdminWorkspace } from "@/components/portal/property-admin-workspace"
import { getPropertyWorkspace } from "@/db/portal-property"

export const dynamic = "force-dynamic"

export default async function PropertyAdminWorkspacePage({
  params,
}: {
  params: Promise<{ propertyId: string }>
}) {
  const { propertyId } = await params
  const workspace = await getPropertyWorkspace(propertyId)

  if (!workspace.property) {
    notFound()
  }

  return <PropertyAdminWorkspace workspace={workspace} />
}

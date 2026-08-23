import { redirect } from "next/navigation"

import { getDeals } from "@/db/deals"
import { getClients } from "@/db/clients"
import { getProperties } from "@/db/properties"
import { listFormInstances } from "@/db/document-form-instance"
import { listTemplates } from "@/lib/forms/template-registry"
import { createFormAction } from "@/app/portal/forms/actions"

export const dynamic = "force-dynamic"

export default async function FormsPage() {
  const instances = await listFormInstances()
  const latest = instances[0]
  if (latest) redirect(`/portal/forms/${latest.id}`)

  const templates = listTemplates()
  const [deals, clients, properties] = await Promise.all([
    getDeals(),
    getClients(),
    getProperties(),
  ])
  const result = await createFormAction({
    templateId: templates[0]?.id ?? "",
    dealId: deals[0]?.id,
    personId: deals[0] ? undefined : clients[0]?.id,
    propertyId: deals[0] || clients[0] ? undefined : properties[0]?.id,
  })
  if (result.ok) redirect(`/portal/forms/${result.data.formId}`)

  return (
    <p className="font-serif text-lg font-light text-[var(--portal-navy)]">
      Add a deal, client, or property first, then open Forms.
    </p>
  )
}

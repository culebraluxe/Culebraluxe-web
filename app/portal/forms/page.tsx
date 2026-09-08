import { getDeals } from "@/db/deals"
import { getClients } from "@/db/clients"
import { getProperties } from "@/db/properties"
import { listFormInstances } from "@/db/document-form-instance"
import {
  getActiveTemplate,
  listPortalFormTypes,
} from "@/lib/forms/template-registry"
import { FormsOverview } from "@/components/portal/forms/forms-overview"

export const dynamic = "force-dynamic"

// Forms — canonical home. The operator chooses one of the four supported
// production form types, binds deal/client/property context, then opens the
// mature editor for that specific form instance.
export default async function FormsPage() {
  const [formTypes, deals, clients, propertiesResult, instances] = await Promise.all([
    Promise.resolve(listPortalFormTypes()),
    getDeals(),
    getClients(),
    getProperties(),
    listFormInstances(),
  ])

  const templates = formTypes.flatMap((formType) => {
    const template = getActiveTemplate(formType.id)
    return template
      ? [{ id: formType.id, displayName: formType.displayName, version: template.version }]
      : []
  })
  const properties = propertiesResult.ok ? propertiesResult.data : []

  return (
    <FormsOverview
      templates={templates}
      deals={deals.map((deal) => ({
        id: deal.id,
        label: deal.clientName
          ? `${deal.clientName} — ${deal.propertyName}`
          : deal.propertyName,
      }))}
      clients={clients.map((client) => ({
        id: client.id,
        label: client.displayName,
      }))}
      properties={properties.map((property) => ({
        id: property.id,
        label: property.location
          ? `${property.name} · ${property.location}`
          : property.name,
      }))}
      instances={instances.map((instance) => ({
        id: instance.id,
        templateId: instance.templateId,
        status: instance.status,
        dealLabel: instance.dealLabel,
        clientName: instance.clientName,
        propertyLabel: instance.propertyLabel,
        updatedAt: instance.updatedAt,
      }))}
    />
  )
}

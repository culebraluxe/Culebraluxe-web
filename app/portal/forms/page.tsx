import { getDeals } from "@/db/deals"
import { getClients } from "@/db/clients"
import { getProperties } from "@/db/properties"
import { listFormInstances } from "@/db/document-form-instance"
import { listTemplates } from "@/lib/forms/template-registry"
import { FormsOverview } from "@/components/portal/forms/forms-overview"

export const dynamic = "force-dynamic"

export default async function FormsPage() {
  const [templates, deals, clients, properties, instances] = await Promise.all([
    Promise.resolve(listTemplates()),
    getDeals(),
    getClients(),
    getProperties(),
    listFormInstances(),
  ])

  return (
    <FormsOverview
      templates={templates.map((t) => ({
        id: t.id,
        displayName: t.displayName,
        version: t.version,
      }))}
      deals={deals.map((d) => ({
        id: d.id,
        label: d.clientName ? `${d.clientName} — ${d.propertyName}` : d.propertyName,
      }))}
      clients={clients.map((c) => ({ id: c.id, label: c.displayName }))}
      properties={properties.map((p) => ({
        id: p.id,
        label: p.location ? `${p.name} · ${p.location}` : p.name,
      }))}
      instances={instances.map((f) => ({
        id: f.id,
        templateId: f.templateId,
        status: f.status,
        dealLabel: f.dealLabel,
        clientName: f.clientName,
        propertyLabel: f.propertyLabel,
        updatedAt: f.updatedAt,
      }))}
    />
  )
}

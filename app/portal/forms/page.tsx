import { getDeals } from "@/db/deals"
import { listFormInstances } from "@/db/document-form-instance"
import { listTemplates } from "@/lib/forms/template-registry"
import { FormsOverview } from "@/components/portal/forms/forms-overview"

export const dynamic = "force-dynamic"

// DOC-07 — NEXUS Forms: approved transaction forms. The board lists form
// instances (mutable working state); issuance hands immutable PDFs to DOC-06
// (/portal/documents). No folders, no filesystem — retrieval is by deal/client.
export default async function FormsPage() {
  const [templates, deals, instances] = await Promise.all([
    Promise.resolve(listTemplates()),
    getDeals(),
    listFormInstances(),
  ])

  return (
    <FormsOverview
      templates={templates.map((t) => ({ id: t.id, displayName: t.displayName, version: t.version }))}
      deals={deals.map((d) => ({ id: d.id, label: d.clientName ? `${d.clientName} — ${d.propertyName}` : d.propertyName }))}
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

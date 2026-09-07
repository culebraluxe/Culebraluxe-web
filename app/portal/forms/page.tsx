import { notFound } from "next/navigation"

import { FormEditorSurface } from "@/components/portal/forms/form-editor-surface"
import { FormLens } from "@/components/portal/form-lens"
import { listFormInstances } from "@/db/document-form-instance"
import {
  getActiveTemplate,
  LISTING_AGREEMENT_TEMPLATE_ID,
} from "@/lib/forms/template-registry"

export const dynamic = "force-dynamic"

// Forms — canonical landing. Promoted from the sidecar FormLens: open the
// active template's mutable draft when one exists, otherwise offer the FormLens
// picker. Legacy auto-create launcher + /portal/form-lens sidecar remain as an
// emergency reference; rollback is one git revert of this file.
export default async function FormsPage() {
  const instances = await listFormInstances()
  const template = getActiveTemplate(LISTING_AGREEMENT_TEMPLATE_ID)
  if (!template) notFound()

  const listings = instances.filter(
    (item) => item.templateId === LISTING_AGREEMENT_TEMPLATE_ID,
  )
  const preferred =
    listings.find(
      (item) =>
        item.templateVersion === template.version && item.status !== "issued",
    ) ??
    listings.find((item) => item.templateVersion === template.version) ??
    listings[0]

  if (preferred) {
    return <FormEditorSurface formId={preferred.id} />
  }

  return <FormLens template={template} />
}

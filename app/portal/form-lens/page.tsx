import { notFound } from "next/navigation"

import { FormEditorSurface } from "@/components/portal/forms/form-editor-surface"
import { FormLens } from "@/components/portal/form-lens"
import { listFormInstances } from "@/db/document-form-instance"
import {
  getActiveTemplate,
  LISTING_AGREEMENT_TEMPLATE_ID,
} from "@/lib/forms/template-registry"

export const dynamic = "force-dynamic"

/**
 * Forms architecture sidecar.
 *
 * Visual/interaction parity is intentionally delegated to the canonical
 * production FormEditor surface. The Form Lens controller/service experiment
 * remains available as the empty-state proving ground instead of maintaining a
 * second editor UI that can drift from production.
 */
export default async function FormLensPage() {
  const instances = await listFormInstances()
  const latestListing = instances.find(
    (item) => item.templateId === LISTING_AGREEMENT_TEMPLATE_ID,
  )

  if (latestListing) {
    return <FormEditorSurface formId={latestListing.id} />
  }

  const template = getActiveTemplate(LISTING_AGREEMENT_TEMPLATE_ID)
  if (!template) notFound()
  return <FormLens template={template} />
}

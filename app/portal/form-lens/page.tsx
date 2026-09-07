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
 * production FormEditor surface. V4 deliberately prefers the active template's
 * mutable draft so opening the sidecar cannot silently drop the operator into a
 * historical Listing Agreement simply because that row was edited more recently.
 */
export default async function FormLensPage() {
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

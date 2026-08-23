import { notFound } from "next/navigation"

import { getFormInstance, listFormInstances } from "@/db/document-form-instance"
import { getIssuedDocumentForFormInstance } from "@/db/issued-document"
import { getTemplate, listPortalFormTypes } from "@/lib/forms/template-registry"
import { FormEditor } from "@/components/portal/forms/form-editor"

export const dynamic = "force-dynamic"

export default async function FormPage({
  params,
}: {
  params: Promise<{ formId: string }>
}) {
  const { formId } = await params
  const form = await getFormInstance(formId)
  if (!form) notFound()
  const template = getTemplate(form.templateId)
  if (!template) notFound()
  const savedForms = await listFormInstances()

  const issuedDocument =
    form.status === "issued"
      ? await getIssuedDocumentForFormInstance(form.id)
      : null

  const templates = [...listPortalFormTypes()]
  if (!templates.some((item) => item.id === template.id)) {
    templates.unshift({ id: template.id, displayName: template.displayName })
  }

  return (
    <FormEditor
      form={{
        id: form.id,
        status: form.status,
        templateId: form.templateId,
        dealId: form.dealId,
        personId: form.personId,
        propertyId: form.propertyId,
        fieldValues: form.fieldValues,
        sections: form.sections,
      }}
      template={template}
      templates={templates}
      savedForms={savedForms.map((item) => ({
        id: item.id,
        templateId: item.templateId,
        clientName: item.clientName,
        propertyLabel: item.propertyLabel,
        buyerName: item.fieldValues.buyerName ?? null,
        sellerName: item.fieldValues.sellerName ?? null,
        updatedAt: item.updatedAt,
      }))}
      issuedDocument={issuedDocument}
    />
  )
}

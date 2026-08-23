import { notFound } from "next/navigation"

import { getFormInstance } from "@/db/document-form-instance"
import { getIssuedDocumentForFormInstance } from "@/db/issued-document"
import { getTemplate, listTemplates } from "@/lib/forms/template-registry"
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

  const issuedDocument =
    form.status === "issued"
      ? await getIssuedDocumentForFormInstance(form.id)
      : null

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
      templates={listTemplates().map((item) => ({
        id: item.id,
        displayName: item.displayName,
      }))}
      issuedDocument={issuedDocument}
    />
  )
}

import { notFound } from "next/navigation"

import { getFormInstance } from "@/db/document-form-instance"
import { getTemplate } from "@/lib/forms/template-registry"
import { OfferLetterForm } from "@/components/portal/forms/offer-letter-form"

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

  return (
    <OfferLetterForm
      form={{
        id: form.id,
        status: form.status,
        fieldValues: form.fieldValues,
        sections: form.sections,
        dealId: form.dealId,
      }}
      template={template}
    />
  )
}

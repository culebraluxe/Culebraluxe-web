import { notFound } from "next/navigation"

import { getFormInstance, listFormInstances } from "@/db/document-form-instance"
import { getIssuedDocumentForFormInstance } from "@/db/issued-document"
import { listSignatureRequestsByDocument } from "@/db/signature-request"
import { listFormSignerPeople } from "@/db/form-signer"
import { getTemplate, listPortalFormTypes } from "@/lib/forms/template-registry"
import { pickFormSigners } from "@/lib/forms/signer-resolution"
import { formContentFingerprint } from "@/lib/forms/artifact-identity"
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

  const issuedDocument = await getIssuedDocumentForFormInstance(form.id)
  const issuedFieldValues = issuedDocument?.sourceSnapshot?.fieldValues
  const issuedSections = issuedDocument?.sourceSnapshot?.sections
  const issuedContentFingerprint =
    issuedFieldValues &&
    typeof issuedFieldValues === "object" &&
    issuedSections &&
    typeof issuedSections === "object"
      ? formContentFingerprint(
          issuedFieldValues as Record<string, string>,
          issuedSections as Record<string, string>,
        )
      : null
  const signatureRequests = issuedDocument
    ? await listSignatureRequestsByDocument(issuedDocument.documentId)
    : []
  const signatureRequest =
    signatureRequests.find((item) =>
      ["requested", "sent", "viewed", "signed"].includes(item.status),
    ) ?? signatureRequests.at(-1) ?? null
  const signerCandidates = pickFormSigners({
    template,
    fieldValues: form.fieldValues,
    people: await listFormSignerPeople(form.id),
  })

  const templates = [...listPortalFormTypes()]
  if (!templates.some((item) => item.id === template.id)) {
    templates.unshift({ id: template.id, displayName: template.displayName })
  }

  return (
    <FormEditor
      key={form.id}
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
        status: item.status,
        clientName: item.clientName,
        propertyLabel: item.propertyLabel,
        buyerName:
          item.fieldValues.buyerName ??
          item.fieldValues.visitorName ??
          null,
        sellerName: item.fieldValues.sellerName ?? null,
        updatedAt: item.updatedAt,
      }))}
      issuedDocument={
        issuedDocument
          ? {
              documentId: issuedDocument.documentId,
              issuedVersion: issuedDocument.issuedVersion,
              checksum: issuedDocument.checksum,
              contentFingerprint: issuedContentFingerprint,
            }
          : null
      }
      signerCandidates={signerCandidates}
      signatureRequest={
        signatureRequest
          ? {
              id: signatureRequest.id,
              status: signatureRequest.status,
            }
          : null
      }
    />
  )
}

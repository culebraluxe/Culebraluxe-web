import { notFound } from "next/navigation"

import { getFormInstance, listFormInstances } from "@/db/document-form-instance"
import { resolveDealLaunchContext } from "@/db/form-service-lineage"
import { getIssuedDocumentForFormInstance } from "@/db/issued-document"
import { listSignatureRequestsByDocument } from "@/db/signature-request"
import { listFormSignerPeople } from "@/db/form-signer"
import { getTemplate, listPortalFormTypes } from "@/lib/forms/template-registry"
import { pickFormSigners } from "@/lib/forms/signer-resolution"
import { isExecutionEligibleTemplate } from "@/lib/agreements/execution"
import { formContentFingerprint } from "@/lib/forms/artifact-identity"
import { loadListingCanonicalSnapshot } from "@/lib/forms/listing-canonical-binding"
import { LISTING_CANONICAL_FIELD_NAMES } from "@/lib/forms/listing-field-binding"
import { hydrateServiceBoundForm } from "@/lib/forms/form-service-hydration"
import { applyTemplateFieldDefaults } from "@/lib/forms/offer-letter-data"
import { FormEditor } from "@/components/portal/forms/form-editor"

/**
 * Canonical Forms working surface loader.
 *
 * The mature FormEditor remains the View. Canonical service-owned facts are
 * composed into that exact working surface instead of maintaining a second
 * sidecar editor that can drift from production behavior.
 */
export async function FormEditorSurface({ formId }: { formId: string }) {
  const form = await getFormInstance(formId)
  if (!form) notFound()

  // A saved form always opens against its exact stored immutable template version.
  const template = getTemplate(form.templateId, form.templateVersion)
  if (!template) notFound()
  const savedForms = await listFormInstances()

  const issuedDocument = await getIssuedDocumentForFormInstance(form.id)

  let editorPersonId = form.personId
  let editorPropertyId = form.propertyId
  let editorFieldValues = form.fieldValues
  let editorSections = form.sections

  // Human-approved template defaults belong to the mutable working draft only.
  // This also repairs older unissued drafts that predate a default (for example
  // OFFER-01 brokerName and SHOW-RPT agentName) without ever reinterpreting an
  // already-issued source snapshot.
  if (!issuedDocument) {
    editorFieldValues = applyTemplateFieldDefaults(template, editorFieldValues)
  }

  // LISTING-01 canonical composition belongs under the proven editor, not in a
  // duplicate screen. Never reinterpret an already-issued artifact: immutable
  // issued bytes and their stored source snapshot remain the historical truth.
  if (template.id === "LISTING-01" && !issuedDocument) {
    try {
      if ((!editorPersonId || !editorPropertyId) && form.dealId) {
        const launch = await resolveDealLaunchContext(form.dealId)
        editorPersonId = editorPersonId ?? launch?.personId ?? null
        editorPropertyId = editorPropertyId ?? launch?.propertyId ?? null
      }

      if (editorPersonId) {
        const canonical = await loadListingCanonicalSnapshot(editorPersonId)
        const templateFieldNames = new Set(template.fields.map((field) => field.name))
        const hydrated = { ...editorFieldValues }

        for (const name of LISTING_CANONICAL_FIELD_NAMES) {
          if (!templateFieldNames.has(name)) continue
          const origin = canonical.origins[name]
          const value = canonical.fields[name]?.trim() ?? ""

          // Person/Property are canonical. Listing-form fallback evidence is
          // deliberately NOT copied from some other saved form; the currently
          // opened form already owns its own saved evidence.
          if ((origin === "person" || origin === "property") && value) {
            hydrated[name] = value
          }
        }

        editorFieldValues = hydrated
        editorPropertyId = editorPropertyId ?? canonical.physicalPropertyId
      }
    } catch (error) {
      // Rollout-safe: PROD may not have migrations 115/116 yet. Falling back to
      // the saved form keeps the existing Forms workflow available while DEV
      // proves the canonical binding before schema promotion.
      console.warn(
        "Listing canonical hydration unavailable; using saved form values.",
        error instanceof Error ? error.message : error,
      )
    }
  }

  // SHOW-RPT and OFFER-01 use the same mature editor but read their canonical
  // state back through ShowingService / ContractService. Deal is only allowed
  // to resolve the explicit Person + Property launch context for older drafts;
  // service lineage itself is never inferred on load.
  if ((template.id === "SHOW-RPT" || template.id === "OFFER-01") && !issuedDocument) {
    try {
      if ((!editorPersonId || !editorPropertyId) && form.dealId) {
        const launch = await resolveDealLaunchContext(form.dealId)
        editorPersonId = editorPersonId ?? launch?.personId ?? null
        editorPropertyId = editorPropertyId ?? launch?.propertyId ?? null
      }

      const hydrated = await hydrateServiceBoundForm(
        {
          ...form,
          personId: editorPersonId,
          propertyId: editorPropertyId,
        },
        editorFieldValues,
        editorSections,
      )
      editorFieldValues = applyTemplateFieldDefaults(template, hydrated.fieldValues)
      editorSections = hydrated.sections
    } catch (error) {
      // Existing pre-refactor drafts remain usable if their canonical service
      // lineage is absent or an environment is still behind the schema rollout.
      console.warn(
        `${template.id} service hydration unavailable; using saved form values.`,
        error instanceof Error ? error.message : error,
      )
    }
  }

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
    fieldValues: editorFieldValues,
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
        personId: editorPersonId,
        propertyId: editorPropertyId,
        fieldValues: editorFieldValues,
        sections: editorSections,
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
      sendAllRequiredSigners={isExecutionEligibleTemplate(template.id)}
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

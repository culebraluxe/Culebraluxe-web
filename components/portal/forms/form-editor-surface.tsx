import { notFound } from "next/navigation"

import { getFormInstance, listFormInstances } from "@/db/document-form-instance"
import { resolveDealLaunchContext } from "@/db/form-service-lineage"
import { getIssuedDocumentForFormInstance } from "@/db/issued-document"
import { listSignatureRequestsByDocument } from "@/db/signature-request"
import { listFormSignerPeople } from "@/db/form-signer"
import {
  getActiveTemplate,
  getTemplate,
  listPortalFormTypes,
} from "@/lib/forms/template-registry"
import { pickFormSigners } from "@/lib/forms/signer-resolution"
import { isExecutionEligibleTemplate } from "@/lib/agreements/execution"
import { formContentFingerprint } from "@/lib/forms/artifact-identity"
import { loadListingCanonicalSnapshot } from "@/lib/forms/listing-canonical-binding"
import { LISTING_CANONICAL_FIELD_NAMES } from "@/lib/forms/listing-field-binding"
import { hydrateServiceBoundForm } from "@/lib/forms/form-service-hydration"
import { applyTemplateFieldDefaults } from "@/lib/forms/offer-letter-data"
import { FormEditor } from "@/components/portal/forms/form-editor"
import { ListingV4Controls } from "@/components/portal/forms/listing-v4-controls"

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
  const activeTemplate = getActiveTemplate(template.id)
  const savedForms = await listFormInstances()

  const issuedDocument = await getIssuedDocumentForFormInstance(form.id)

  let editorPersonId = form.personId
  let editorPropertyId = form.propertyId
  let editorFieldValues = form.fieldValues
  let editorSections = form.sections

  // Human-approved template defaults belong to the mutable working draft only.
  // This also repairs older unissued drafts that predate a default without ever
  // reinterpreting an already-issued source snapshot.
  if (!issuedDocument) {
    editorFieldValues = applyTemplateFieldDefaults(template, editorFieldValues)
  }

  // LISTING-01 canonical composition belongs under the proven editor, not in a
  // duplicate screen. V4 automatic hydration fills blanks only: a value already
  // present in this working draft is treated as deliberate and never clobbered
  // merely because Person/Property currently says something different.
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
          const current = hydrated[name]?.trim() ?? ""

          if (
            (origin === "person" || origin === "property") &&
            value &&
            !current
          ) {
            hydrated[name] = value
          }
        }

        editorFieldValues = hydrated
        editorPropertyId = editorPropertyId ?? canonical.physicalPropertyId
      }
    } catch (error) {
      // Rollout-safe: an environment behind canonical schema still gets the
      // saved form rather than losing the proven Forms workflow.
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

  // Active-version drafts sort ahead of historical rows. We keep the proven
  // Forms rail itself untouched and add version/history clarity to its existing
  // subtitle so an old v1/v3 record cannot masquerade as the current template.
  const orderedSavedForms = [...savedForms].sort((left, right) => {
    if (left.templateId !== right.templateId) return 0
    const activeVersion = getActiveTemplate(left.templateId)?.version ?? null
    const leftActive = left.templateVersion === activeVersion
    const rightActive = right.templateVersion === activeVersion
    if (leftActive !== rightActive) return leftActive ? -1 : 1
    return right.updatedAt.localeCompare(left.updatedAt)
  })

  return (
    <div className="flex min-h-0 flex-col gap-3">
      {template.id === "LISTING-01" ? (
        <ListingV4Controls
          formId={form.id}
          personId={editorPersonId}
          sellerName={editorFieldValues.sellerName ?? ""}
          templateVersion={template.version}
          activeTemplateVersion={activeTemplate?.version ?? template.version}
          status={form.status}
          issued={Boolean(issuedDocument)}
        />
      ) : null}

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
        savedForms={orderedSavedForms.map((item) => {
          const itemActiveVersion = getActiveTemplate(item.templateId)?.version ?? item.templateVersion
          const history = item.templateVersion !== itemActiveVersion
          const versionLabel = `v${item.templateVersion}${history ? " · history" : ""}`
          return {
            id: item.id,
            templateId: item.templateId,
            status: item.status,
            clientName: item.clientName,
            propertyLabel: [item.propertyLabel, versionLabel]
              .filter(Boolean)
              .join(" · "),
            buyerName:
              item.fieldValues.buyerName ??
              item.fieldValues.visitorName ??
              null,
            sellerName: item.fieldValues.sellerName ?? null,
            updatedAt: item.updatedAt,
          }
        })}
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
    </div>
  )
}

import { redirect } from "next/navigation"

import { listIssuedDocuments } from "@/db/transaction-document"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { DocumentList } from "@/components/portal/documents/document-list"

export const dynamic = "force-dynamic"

// DOC-06 — NEXUS Documents: the canonical issued-document repository.
// Retrieval is by metadata (deal, client, property, document type, version),
// never folder-first. Only issued (generated) artifacts appear here.
export default async function DocumentsPage() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "deal.read",
  )
  if (!access.ok) redirect(access.redirectTo)

  const documents = await listIssuedDocuments(
    undefined,
    access.actor
      ? { accountType: access.actor.accountType, personId: access.actor.personId }
      : undefined,
  )

  return (
    <DocumentList
      documents={documents.map((d) => ({
        id: d.id,
        documentTypeLabel: d.documentTypeLabel ?? "Document",
        title: d.title,
        state: d.state,
        templateId: d.templateId,
        issuedVersion: d.issuedVersion,
        issuedChecksumSha256: d.issuedChecksumSha256,
        issuedByDisplayName: d.issuedByDisplayName,
        partyName: d.partyName,
        propertyName: d.propertyName,
        dealName: d.dealName,
        createdAt: d.createdAt,
      }))}
    />
  )
}

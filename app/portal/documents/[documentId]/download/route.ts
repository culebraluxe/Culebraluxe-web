import { getTransactionDocument, listIssuedDocuments } from "@/db/transaction-document"
import { getMediaBytes } from "@/db/issued-document"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// DOC-06 — auth-gated issued-PDF download. Repository artifacts are NEVER on
// guessable public URLs: this route requires the authenticated portal session
// + deal.read, and external actors are scoped to their own deals.
export async function GET(
  request: Request,
  { params }: { params: Promise<{ documentId: string }> },
) {
  const { documentId } = await params
  const inline = new URL(request.url).searchParams.get("inline") === "1"
  const audit = new URL(request.url).searchParams.get("artifact") === "audit"

  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "deal.read",
  )
  if (!access.ok) {
    return new Response("Unauthorized", { status: 401 })
  }

  const document = await getTransactionDocument(documentId)
  if (!document || !document.mediaId) {
    return new Response("Not found", { status: 404 })
  }

  // AUTH-02 external scoping: only issued documents for the actor's own deals.
  if (access.actor.accountType === "external") {
    const scoped = await listIssuedDocuments(undefined, {
      accountType: access.actor.accountType,
      personId: access.actor.personId,
    })
    if (!scoped.some((d) => d.id === documentId)) {
      return new Response("Not found", { status: 404 })
    }
  }

  const mediaId = audit
    ? document.signedAuditMediaId
    : document.signedMediaId ?? document.mediaId
  if (!mediaId) {
    return new Response("Not found", { status: 404 })
  }
  const media = await getMediaBytes(mediaId)
  if (!media) {
    return new Response("Not found", { status: 404 })
  }

  return new Response(new Uint8Array(media.bytes), {
    headers: {
      "Content-Type": media.mimeType || "application/pdf",
      "Content-Disposition": `${inline ? "inline" : "attachment"}; filename="${media.filename}"`,
      "Content-Length": String(media.bytes.length),
    },
  })
}

// SEC-MEDIA-DOC-01 — the ONE document-download access decision.
//
// Downloading a document is gated by exactly this function: session facts in,
// allow | deny with a reason out. It is pure on purpose — no I/O, no env, no
// database, no framework imports — so the rule cannot disagree with itself in
// two places and a test can interrogate it directly. The route owns the facts;
// this function owns the rule.

export type DocumentAccessReason = 'not_found' | 'unauthenticated'

export type DocumentAccessFacts = {
  /** True only when the requested media row exists, is a document, and has bytes. */
  isDocument: boolean
  /** True only when the request carries a valid authenticated portal session. */
  hasPortalSession: boolean
}

export type DocumentAccessDecision =
  | { allow: true }
  | { allow: false; reason: DocumentAccessReason }

export function decideDocumentAccess(
  facts: DocumentAccessFacts,
): DocumentAccessDecision {
  // A non-document (or missing) media id stays 404 — it is never revealed as
  // "needs auth", and a document id is never confused with one.
  if (!facts.isDocument) return { allow: false, reason: 'not_found' }
  // An executed contract is not served to whoever holds its id: a document with
  // no portal session returns 401 rather than a 404 that hides the rule.
  if (!facts.hasPortalSession) return { allow: false, reason: 'unauthenticated' }
  return { allow: true }
}

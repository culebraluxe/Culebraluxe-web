import { NextRequest, NextResponse } from "next/server"

import {
  classifyEvidenceRow,
  getRelationshipEvidenceById,
  recordReconcileDecision,
  rerunRelationshipReconciliation,
} from "@/db/relationship-evidence"
import { personExists } from "@/db/person-identities"
import { recordSecurityAuditEvent } from "@/db/security-audit"
import { REL_INTEL_RULE_VERSION } from "@/lib/relationship-intel/reconcile"
import type { ReviewState } from "@/lib/relationship-intel/contracts"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

// ---------------------------------------------------------------------------
// REL-INTEL — OPPS relationship-evidence stewardship actions.
//
// Occasional, bounded, human stewardship over reconciliation exceptions. All
// actions require the authenticated portal session + crm.write. Identity-changing
// actions (link / reject) additionally require explicit confirmation and write a
// durable security-audit event. Promotion/links go through the sanctioned
// recordReconcileDecision seam (review fields) — never a direct canonical table
// write. This is NOT a generic CRM CRUD surface.
// ---------------------------------------------------------------------------

export const dynamic = "force-dynamic"

const VALID_REVIEW_STATES = [
  "unresolved", "exact_linked", "review_required", "ambiguous", "unmatched",
  "rejected", "non_person", "deferred",
]

export async function POST(req: NextRequest) {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "crm.write")
  if (!access.ok) {
    return NextResponse.json({ ok: false, code: "unauthorized", message: "Unauthorized" }, { status: 401 })
  }
  const actor = access.actor

  let body: Record<string, unknown>
  try {
    body = await req.json()
  } catch {
    return NextResponse.json({ ok: false, code: "invalid", message: "Invalid JSON body" }, { status: 400 })
  }

  const action = typeof body.action === "string" ? body.action : ""
  const id = typeof body.id === "string" ? body.id : ""

  const fail = (code: string, message: string, status = 400) =>
    NextResponse.json({ ok: false, code, message }, { status })

  try {
    // inspect — read one evidence row (reason / provenance), no mutation.
    if (action === "inspect") {
      if (!id) return fail("validation", "id is required")
      const row = await getRelationshipEvidenceById(id)
      if (!row) return fail("not-found", "Evidence row not found", 404)
      return NextResponse.json({ ok: true, row })
    }

    // classify_automated / classify_service — correct source classification, then
    // safely re-run reconciliation for that single row.
    if (action === "classify_automated" || action === "classify_service") {
      if (!id) return fail("validation", "id is required")
      await classifyEvidenceRow(
        id,
        action === "classify_automated"
          ? { isAutomatedOrBulk: true }
          : { isOrganizationOrService: true },
      )
      const { rows, tally, canonicalLinked } = await rerunRelationshipReconciliation({ ids: [id], limit: 1 })
      return NextResponse.json({ ok: true, tally, canonicalLinked, row: rows[0] ?? null })
    }

    // link — approve a legitimate source-to-Person link via the sanctioned seam.
    if (action === "link") {
      if (!id) return fail("validation", "id is required")
      const personId = typeof body.personId === "string" ? body.personId : ""
      if (!personId) return fail("validation", "personId is required")
      if (body.confirm !== true) return fail("confirmation", "Explicit confirmation required", 409)
      const exists = await personExists(personId)
      if (!exists) return fail("not-found", "Person not found", 404)
      await recordReconcileDecision(
        id,
        {
          reviewState: "exact_linked",
          matchMethod: "source_link",
          matchConfidence: "exact",
          canonicalPersonId: personId,
          reason: "opps_operator_approval",
          ruleVersion: REL_INTEL_RULE_VERSION,
        },
      )
      await recordSecurityAuditEvent({
        appUserId: actor.appUserId,
        eventType: "relationship_evidence.opps_link",
        authenticationMethod: "portal",
        metadata: { evidenceId: id, canonicalPersonId: personId },
      })
      const row = await getRelationshipEvidenceById(id)
      return NextResponse.json({ ok: true, row })
    }

    // reject — dismiss an invalid/undesired match (never silent merge).
    if (action === "reject") {
      if (!id) return fail("validation", "id is required")
      if (body.confirm !== true) return fail("confirmation", "Explicit confirmation required", 409)
      await recordReconcileDecision(
        id,
        {
          reviewState: "rejected",
          matchMethod: "rejected",
          matchConfidence: "none",
          canonicalPersonId: null,
          reason: "opps_operator_dismissal",
          ruleVersion: REL_INTEL_RULE_VERSION,
        },
      )
      await recordSecurityAuditEvent({
        appUserId: actor.appUserId,
        eventType: "relationship_evidence.opps_reject",
        authenticationMethod: "portal",
        metadata: { evidenceId: id },
      })
      const row = await getRelationshipEvidenceById(id)
      return NextResponse.json({ ok: true, row })
    }

    // rerun — safely re-run deterministic reconciliation over a bounded subset.
    if (action === "rerun") {
      const source = typeof body.source === "string" ? body.source : undefined
      const reviewState =
        typeof body.reviewState === "string" && VALID_REVIEW_STATES.includes(body.reviewState)
          ? (body.reviewState as ReviewState)
          : undefined
      const limit = typeof body.limit === "number" ? body.limit : 200
      const result = await rerunRelationshipReconciliation({ source, reviewState, limit })
      return NextResponse.json({ ok: true, ...result })
    }

    return fail("validation", `Unknown action: ${action}`)
  } catch (error) {
    console.error("REL-INTEL OPPS action failed.", error instanceof Error ? error.message : error)
    return fail("unknown", "Something went wrong. Please try again.", 500)
  }
}


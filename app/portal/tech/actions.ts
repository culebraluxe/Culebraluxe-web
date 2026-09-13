"use server"
import { withServerErrorCapture } from '@/lib/error-capture-seam'

import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { setActiveWork, setStoryboardStatus } from "@/db/storyboard"
import {
  STATUS_BY_BUCKET,
  canMove,
  type StoryBucket,
} from "@/lib/story-moves"

// PORTAL-13 — Active Queue selection seam (TECH surface). Selecting/removing a
// story from Active Work is an INTENT flag only: it never changes story status,
// never touches Forge run state, never creates an agent work item, and never
// launches work. Requires tech.access.
async function setActiveWorkActionHandler(formData: FormData): Promise<void> {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const storyId = String(formData.get("storyId") ?? "").trim()
  const active = formData.get("active") === "true"
  if (!storyId) return
  await setActiveWork(storyId, active, access.ok ? access.actor.appUserId : null)
}

// ENG-FORGE error-capture: a throw is recorded durably, then rethrown.
export const setActiveWorkAction = withServerErrorCapture('portal/tech/actions.setActiveWorkAction', setActiveWorkActionHandler)

// ---------------------------------------------------------------------------
// SORTER MOVES — the write behind dragging a story between buckets.
//
// Wired today: BACKLOG / OPEN / CLOSED / NEXT VERSION (a real status) and WORK
// BENCH (an intent row in storyboard_active_work — it never changes status, which
// is why leaving the bench has to CLEAR that row explicitly rather than relying on
// the status write).
//
// DELIBERATELY NOT WIRED: ENGINE QUEUE. Handing a story to Forge dispatches real
// work, and this repo's own rule is that a drag is not the place for that. The
// board gets a clear refusal instead of a silent no-op, and the gesture stays a
// gesture until the engine path is wired on purpose.
// ---------------------------------------------------------------------------
async function moveStoryBucketActionHandler(
  cardId: string,
  from: string,
  to: string,
): Promise<{ ok: boolean; error?: string }> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const storyId = String(cardId ?? "").trim()
  const source = String(from ?? "").trim() as StoryBucket
  const target = String(to ?? "").trim() as StoryBucket
  if (!storyId) return { ok: false, error: "missing story id" }

  if (!canMove(source, target)) {
    return { ok: false, error: `Not allowed: ${source} → ${target}` }
  }

  const actorId = access.ok ? access.actor.appUserId : null

  // The bench is orthogonal to status: leaving it must clear the intent row.
  if (source === "bench" && target !== "bench") {
    await setActiveWork(storyId, false, actorId)
  }

  if (target === "engine") {
    return {
      ok: false,
      error: "ENGINE QUEUE is not wired yet — handing a story to Forge is a deliberate action, not a drag",
    }
  }

  if (target === "bench") {
    await setActiveWork(storyId, true, actorId)
    return { ok: true }
  }

  const status = STATUS_BY_BUCKET[target]
  if (!status) return { ok: false, error: `Unsupported bucket: ${target}` }

  await setStoryboardStatus(storyId, status)
  return { ok: true }
}

export const moveStoryBucketAction = withServerErrorCapture(
  "portal/tech/actions.moveStoryBucketAction",
  moveStoryBucketActionHandler,
)


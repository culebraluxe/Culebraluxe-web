"use server"
import { withServerErrorCapture } from '@/lib/error-capture-seam'

import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { setActiveWork, setStoryboardStatus } from "@/db/storyboard"
import {
  ENGINE_DISPATCH_STATUS,
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

// WIRED 2026-09-12 (captain's go): ENGINE QUEUE writes ENGINE_DISPATCH_STATUS —
// which is `Ready`, and `Ready` is the DISPATCH TRIGGER (`agent_work_item_dispatch()`
// fires on a status change to Ready and inserts the work item). So this column is
// the handoff, and the consequence is explicit and intended: a drop here QUEUES
// REAL FORGE WORK in PROD. It is the only bucket whose write starts something.
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

  if (target === "bench") {
    await setActiveWork(storyId, true, actorId)
    return { ok: true }
  }

  if (target === "engine") {
    // Deliberate: this is the dispatch. The status change creates the work item.
    await setStoryboardStatus(storyId, ENGINE_DISPATCH_STATUS)
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


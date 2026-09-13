"use server"
import { withServerErrorCapture } from '@/lib/error-capture-seam'

import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { setActiveWork, setStoryboardStatus } from "@/db/storyboard"
import { setAgentWorkDispatchOptions } from "@/db/agent-work"
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

// ---------------------------------------------------------------------------
// Queue a SCOPED run from the Work Bench — "send this one to Scout" / "to
// Architect" — and park there, so the operator can read the findings on the
// bench before deciding whether the story is worth the tokens.
//
// This is the shape the bench was always meant to have: the operator reviews
// BEFORE Scout (is it worth it? does it add something useful?), then decides.
// A phase run does NOT touch the bench row, so the story stays on the bench and
// stays the operator's until they hand it over for a full run.
//
// The scope lives on the WORK ITEM (migration 167), not on the story: it describes
// THIS dispatch. Writing the status is what queues the work (`Ready` is the
// dispatch trigger, which inserts an `agent_work_item`), so the scope is written
// onto the item the trigger just created — and re-writing it for an already-queued
// story is how you change your mind before the kick without queuing a second item.
// ---------------------------------------------------------------------------
export type ScopedRunTarget = "scout" | "architect" | "lead"
export type LaunchIntent = "SOLO" | "SMITH" | "SPLIT" | "HOLD" | null

async function queueScopedRunActionHandler(
  input: { storyId: string; stopAfter: ScopedRunTarget; launchIntent?: LaunchIntent },
): Promise<{ ok: boolean; error?: string }> {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const storyId = String(input.storyId ?? "").trim()
  if (!storyId) return { ok: false, error: "missing story id" }

  const stopAfter = String(input.stopAfter ?? "").trim()
  if (stopAfter !== "scout" && stopAfter !== "architect" && stopAfter !== "lead") {
    return { ok: false, error: `unsupported stop-after: ${stopAfter}` }
  }

  const launchIntent = input.launchIntent ?? null
  if (
    launchIntent !== null &&
    !["SOLO", "SMITH", "SPLIT", "HOLD"].includes(launchIntent)
  ) {
    return { ok: false, error: `unsupported launch intent: ${String(launchIntent)}` }
  }

  // Queue it. For a story not already queued this is the dispatch write; for one
  // already waiting on the engine it is a no-op status write, and the options
  // below are what actually change.
  await setStoryboardStatus(storyId, ENGINE_DISPATCH_STATUS)

  const updated = await setAgentWorkDispatchOptions(storyId, { stopAfter, launchIntent })
  if (updated === 0) {
    // The status write did not produce a QUEUED item: the story is already
    // claimed/running (or the queue is unavailable). Do not pretend it was queued.
    return {
      ok: false,
      error: "No queued work item to scope — this story is already running.",
    }
  }
  return { ok: true }
}

export const queueScopedRunAction = withServerErrorCapture(
  "portal/tech/actions.queueScopedRunAction",
  queueScopedRunActionHandler,
)


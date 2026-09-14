"use server"
import { withServerErrorCapture } from '@/lib/error-capture-seam'

import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { setActiveWork, setStoryboardStatus, listStoryIdsWithStatus, listActiveWork, clearActiveWork } from "@/db/storyboard"
import { setAgentWorkDispatchOptions } from "@/db/agent-work"
import {
  ENGINE_DISPATCH_STATUS,
  STATUS_BY_BUCKET,
  canMove,
  normalizeStoryBucket,
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
// CLEAR THE WORK BENCH — the captain's own QA ("move the stories in active work
// bench back to open to clear the junk").
//
// NOT the same as dragging each card to OPEN. OPEN is a STATUS write (`In Progress`),
// and the bench held five `Complete` stories: clearing them that way would have
// UN-FINISHED finished work to tidy a list. The bench is an intent row and nothing
// else, so this removes the intent and leaves every status exactly as the engine
// left it. Reversible: add them back to the bench and nothing was lost.
// ---------------------------------------------------------------------------
async function clearWorkBenchActionHandler(): Promise<{
  ok: boolean
  cleared?: number
  error?: string
}> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const before = await listActiveWork()
  if (before.length === 0) return { ok: true, cleared: 0 }

  try {
    const cleared = await clearActiveWork()
    return { ok: true, cleared }
  } catch (error) {
    return { ok: false, error: String((error as Error)?.message ?? error) }
  }
}

export const clearWorkBenchAction = withServerErrorCapture(
  'portal/tech/actions.clearWorkBenchAction',
  clearWorkBenchActionHandler,
)


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
  if (!storyId) return { ok: false, error: "missing story id" }

  // NORMALIZE AT THE BOUNDARY. A column id is a VIEW name (`next-version`), a bucket is a RULE name
  // (`next`); casting one to the other let `next-version` through as if it were valid and refused
  // every drag out of NEXT VERSION with a rule that looked arbitrary. See normalizeStoryBucket.
  const source = normalizeStoryBucket(from)
  const target = normalizeStoryBucket(to)
  if (!source) return { ok: false, error: `Unknown column: ${from}` }
  if (!target) return { ok: false, error: `Unknown column: ${to}` }

  if (!canMove(source, target)) {
    return { ok: false, error: `Not allowed: ${source} → ${target}` }
  }

  const actorId = access.ok ? access.actor.appUserId : null

  // The bench is orthogonal to status: leaving it must clear the intent row. And STAGING OR RUNNING A
  // STORY TAKES IT OFF THE DAILY BENCH whatever column it came from: you are not working something by
  // hand that you just handed to the engine. (Without this, a story could sit on the bench AND in
  // ENGINE BATCH — one story in two columns, the ambiguity that moved the wrong card.)
  if ((source === 'bench' && target !== 'bench') || target === 'batch' || target === 'engine') {
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
// SEND THE ENGINE BATCH — the deliberate, on-demand dispatch.
//
// Stories staged in ENGINE BATCH carry status 'Batched', which does NOT dispatch anything. This is
// the act that does: for every staged story it writes ENGINE_DISPATCH_STATUS ('Ready'), and the
// `agent_work_item_dispatch()` trigger queues a real work item for each one. It is the same write the
// single-story handoff makes, applied to the whole batch, and it is why the batch exists - stage
// freely, then send when you mean it.
//
// Partial success is reported honestly: if three of five dispatch and two are refused, the operator
// gets "3 queued" and the names that failed, not a boolean.
// ---------------------------------------------------------------------------
async function sendEngineBatchActionHandler(): Promise<{
  ok: boolean
  queued: number
  failed: Array<{ storyId: string; error: string }>
  error?: string
}> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const staged = await listStoryIdsWithStatus(STATUS_BY_BUCKET.batch ?? "Batched")
  if (staged.length === 0) return { ok: true, queued: 0, failed: [] }

  const failed: Array<{ storyId: string; error: string }> = []
  let queued = 0
  for (const storyId of staged) {
    try {
      await setStoryboardStatus(storyId, ENGINE_DISPATCH_STATUS)
      queued += 1
    } catch (error) {
      failed.push({ storyId, error: String((error as Error)?.message ?? error) })
    }
  }
  return { ok: failed.length === 0, queued, failed }
}

export const sendEngineBatchAction = withServerErrorCapture(
  "portal/tech/actions.sendEngineBatchAction",
  sendEngineBatchActionHandler,
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


"use server"
import { withServerErrorCapture } from '@/lib/error-capture-seam'

import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { setActiveWork, setStoryboardStatus, listStoryIdsWithStatus, listActiveWork, clearActiveWork, getStoryboardStory } from "@/db/storyboard"
import type { StoryBucket } from "@/lib/story-moves"
import { storyLifecycleOf } from "@/lib/storyboard-data"
import { setAgentWorkDispatchOptions, withdrawQueuedAgentWork } from "@/db/agent-work"
import {
  cancelForgeBatch,
  fireStagingBatch,
  scheduleStagingBatch,
  stageStoryForBatch,
  unstageStoryForBatch,
} from "@/db/forge-batch"
import {
  ENGINE_DISPATCH_STATUS,
  STATUS_BY_BUCKET,
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
): Promise<{ ok: boolean; error?: string; note?: string }> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const storyId = String(cardId ?? "").trim()
  if (!storyId) return { ok: false, error: "missing story id" }

  // NORMALIZE AT THE BOUNDARY. A column id is a VIEW name (`next-version`), a bucket is a RULE name
  // (`next`); casting one to the other let `next-version` through as if it were valid and refused
  // every drag out of NEXT VERSION with a rule that looked arbitrary. See normalizeStoryBucket.
  const target = normalizeStoryBucket(to)
  if (!target) return { ok: false, error: `Unknown column: ${to}` }

  // THE SOURCE CAN ALWAYS BE KNOWN, so a move is never refused for want of it.
  //
  // The board's card carries its column, but the vendor's passive move event reports only the TARGET
  // (`StoreActions['move-card'] = { id, column?, before? }`), so a perfectly good drag could arrive
  // with the origin missing - and the old code answered that with "Unknown column", which is a refusal
  // the operator can do nothing about. The DATABASE always knows where a story really is, so derive it
  // here: bench membership first (the bench is an intent row, not a status), then the status itself.
  const source = normalizeStoryBucket(from) ?? (await deriveSourceBucket(storyId))
  if (!source) return { ok: false, error: `could not tell where ${storyId} came from` }

  // NO GATE. There is deliberately no table of forbidden pairs here: a sticky note goes wherever the
  // captain puts it (2026-09-14). The two honest refusals below are about NAMES, not permissions — an
  // unknown column cannot be written — and a no-op move writes nothing.
  if (source === target) return { ok: true, note: 'already there' }

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
    return { ok: true, note: await leavingEngineNote(storyId, source, target) }
  }

  // LEAVING ENGINE BATCH TAKES THE ROW WITH THE CARD. The board and the table stay in step, so the run
  // never guesses membership from a status: "if it's in the table it goes" only holds if leaving the
  // column removes the row.
  if (source === "batch" && target !== "batch") {
    await unstageStoryForBatch(storyId)
  }

  if (target === "engine") {
    // Deliberate: this is the dispatch. The status change creates the work item.
    await setStoryboardStatus(storyId, ENGINE_DISPATCH_STATUS)
    return { ok: true, note: "handed to the engine — status Ready queued a real work item" }
  }

  const status = STATUS_BY_BUCKET[target]
  if (!status) return { ok: false, error: `Unsupported bucket: ${target}` }

  if (source === "engine") {
    // PULLING THE NOTE BACK TAKES THE REQUEST BACK. Otherwise the engine would run a story the board
    // no longer shows, which is the one way "no gate" could cause real damage. A story the engine is
    // already executing is reported rather than silently yanked.
    const { withdrawn, live } = await withdrawQueuedAgentWork(storyId)
    await setStoryboardStatus(storyId, status)
    const parts: string[] = []
    if (withdrawn > 0) parts.push(`withdrew ${withdrawn} queued engine request${withdrawn === 1 ? '' : 's'}`)
    if (live > 0) parts.push(`the engine is ALREADY RUNNING this — the run continues`)
    return { ok: true, note: parts.join(' · ') || undefined }
  }

  await setStoryboardStatus(storyId, status)

  // STAGING WRITES THE TABLE (the Kahnban -> table sync). By the time the card is drawn in ENGINE BATCH
  // there is a `forge_batch_item` row for it, so "Run batch now" fires the ROWS rather than re-deriving
  // a list from statuses and hoping the two agree.
  if (target === "batch") {
    const staged = await stageStoryForBatch(storyId, actorId)
    return { ok: true, note: `staged in the table (${staged.members} in the batch)` }
  }

  return { ok: true }
}

/**
 * WHERE IS THIS STORY RIGHT NOW, according to the database?
 *
 * Used when the gesture cannot say (the vendor's move event reports only the destination). Order
 * matters: the bench is an INTENT row and beats the status; `Ready` means the engine has it even though
 * `Ready` maps to the `open` lifecycle; `Batched` stages. Everything else reads straight off the status.
 */
async function deriveSourceBucket(storyId: string): Promise<StoryBucket | null> {
  const story = await getStoryboardStory(storyId).catch(() => null)
  if (!story) return null
  const onBench = (await listActiveWork()).some((s) => s.id === storyId)
  if (onBench) return 'bench'
  if (story.status === (STATUS_BY_BUCKET.batch ?? 'Batched')) return 'batch'
  if (story.status === ENGINE_DISPATCH_STATUS) return 'engine'
  const lifecycle = storyLifecycleOf(story.status)
  if (lifecycle === 'backlog') return 'backlog'
  if (lifecycle === 'closed') return 'closed'
  if (lifecycle === 'next-version') return 'next'
  return 'open'
}

/** What to say when a story is pulled back out of the run queue. */
async function leavingEngineNote(
  storyId: string,
  source: string,
  target: string,
): Promise<string | undefined> {
  if (source !== "engine" || target === "engine") return undefined
  const { withdrawn, live } = await withdrawQueuedAgentWork(storyId)
  const parts: string[] = []
  if (withdrawn > 0) parts.push(`withdrew ${withdrawn} queued engine request${withdrawn === 1 ? '' : 's'}`)
  if (live > 0) parts.push("the engine is ALREADY RUNNING this — the run continues")
  return parts.join(' · ') || undefined
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
  batchId?: string
  error?: string
}> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  // THE TABLE IS THE JOB STREAM. `fireStagingBatch` reads `forge_batch_item` (after sweeping up any
  // story staged before the table existed), so what runs is exactly what the board wrote — the count in
  // the button and the count that fires come from the same rows.
  const result = await fireStagingBatch()
  if (!result) return { ok: true, queued: 0, failed: [] }
  return { ok: result.failed.length === 0, queued: result.queued, failed: result.failed, batchId: result.batchId }
}

export const sendEngineBatchAction = withServerErrorCapture(
  "portal/tech/actions.sendEngineBatchAction",
  sendEngineBatchActionHandler,
)

// ---------------------------------------------------------------------------
// SCHEDULE THE ENGINE BATCH — "these are ready to go but i dont want to run them yet, maybe save for a
// night run when its cheaper to run" (the captain, 2026-09-14).
//
// The batch is recorded NOW and `scheduled_for` decides when it fires. Nothing is dispatched at this
// point: the staged stories keep status `Batched`, so Batch still fires nothing. The firing happens in
// the unattended worker pass (`fireDueForgeBatches()` at the top of `agent:work`), which the launchd
// scheduler already wakes every 3 minutes - so a 02:00 batch runs at 02:00 with nobody awake.
// ---------------------------------------------------------------------------
async function scheduleEngineBatchActionHandler(
  scheduledForIso: string,
  label?: string,
): Promise<{ ok: boolean; batchId?: string; stories?: number; scheduledFor?: string; error?: string }> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const when = new Date(String(scheduledForIso ?? ""))
  if (Number.isNaN(when.getTime())) {
    return { ok: false, error: `not a time I can read: ${scheduledForIso}` }
  }

  const staged = await listStoryIdsWithStatus(STATUS_BY_BUCKET.batch ?? "Batched")
  if (staged.length === 0) {
    return { ok: false, error: "nothing is staged in ENGINE BATCH yet" }
  }

  // Same row, same membership — scheduling only puts a TIME on the staging batch, so a scheduled run
  // and an immediate one differ in when, not in what.
  const batch = await scheduleStagingBatch(
    when.toISOString(),
    access.actor.appUserId,
    label?.trim() ? label.trim() : `night run ${when.toISOString().slice(0, 16).replace('T', ' ')}`,
  )
  return { ok: true, batchId: batch.id, stories: batch.storyCount, scheduledFor: batch.scheduledFor ?? undefined }
}

export const scheduleEngineBatchAction = withServerErrorCapture(
  "portal/tech/actions.scheduleEngineBatchAction",
  scheduleEngineBatchActionHandler,
)

/** Abandon a scheduled batch before it fires. Nothing was dispatched, so nothing is undone. */
async function cancelScheduledBatchActionHandler(
  batchId: string,
): Promise<{ ok: boolean; cancelled?: number; error?: string }> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)
  const cancelled = await cancelForgeBatch(String(batchId ?? "").trim())
  if (cancelled === 0) return { ok: false, error: "that batch is not waiting to fire" }
  return { ok: true, cancelled }
}

export const cancelScheduledBatchAction = withServerErrorCapture(
  "portal/tech/actions.cancelScheduledBatchAction",
  cancelScheduledBatchActionHandler,
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


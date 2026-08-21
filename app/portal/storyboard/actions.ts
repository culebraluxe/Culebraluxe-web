'use server'

import { revalidatePath } from 'next/cache'

import { PortalWriteError } from '@/lib/portal-write-error'
import {
  STORY_PRIORITIES,
  STORY_STATUSES,
  WORKSTREAM_CODES,
  type StoryPriority,
  type StoryStatus,
} from '@/lib/storyboard-data'
import {
  createStoryboardStory,
  finishStoryRun,
  setStoryboardStatus,
  startStoryRun,
  updateStoryboardStory,
  type FinishRunInput,
  type StoryboardStory,
  type StoryboardStoryInput,
  type StoryRun,
} from '@/db/storyboard'

// ---------------------------------------------------------------------------
// Story Board write actions. Validation happens here (per Portal convention);
// SQL stays in db/storyboard.ts. Story IDs are human-assigned — the create
// action accepts whatever ID the operator types (e.g. CRM-19, OPS-07, PX-27)
// and the database primary key prevents duplicates.
// ---------------------------------------------------------------------------

export type StoryBoardWriteResult<T> =
  | { ok: true; data: T }
  | { ok: false; code: 'validation' | 'conflict' | 'not-found' | 'unknown'; message: string }

export type StoryFormInput = {
  id: string
  workstream: string
  title: string
  priority: string
  status: string
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  acceptanceCriteria: string | null
  dependencies: string | null
  completion: number
  rollup: boolean
  plannedStartAt: string | null
  actualStartAt: string | null
  completedAt: string | null
}

/** Outcome statuses allowed on a run (Planned / In Progress are not outcomes). */
export const RUN_RESULT_STATUSES = [
  'Complete',
  'Partial',
  'Blocked',
  'Failed',
  'Deferred',
  'Hold',
] as const

export type RunResultStatus = (typeof RUN_RESULT_STATUSES)[number]

function failure(
  error: unknown,
): { ok: false; code: 'validation' | 'conflict' | 'not-found' | 'unknown'; message: string } {
  if (error instanceof PortalWriteError) {
    return { ok: false, code: error.code, message: error.message }
  }
  console.error(
    'Story Board write failed.',
    error instanceof Error ? error.message : error,
  )
  return {
    ok: false,
    code: 'unknown',
    message: 'Something went wrong. Please try again.',
  }
}

function validateId(id: string): string | null {
  const value = id.trim()
  if (!value) return 'Story ID is required.'
  if (value.length > 64) return 'Story ID must be 64 characters or fewer.'
  return null
}

function validateInput(
  input: StoryFormInput,
  requireId: boolean,
): string | null {
  if (requireId) {
    const idError = validateId(input.id)
    if (idError) return idError
  }
  if (!WORKSTREAM_CODES.includes(input.workstream)) {
    return 'Workstream is required.'
  }
  if (!input.title.trim()) return 'Story title is required.'
  if (!STORY_PRIORITIES.includes(input.priority as StoryPriority)) {
    return 'Priority is required.'
  }
  if (!STORY_STATUSES.includes(input.status as StoryStatus)) {
    return 'Status is required.'
  }
  if (
    typeof input.completion !== 'number' ||
    !Number.isInteger(input.completion) ||
    input.completion < 0 ||
    input.completion > 100
  ) {
    return 'Completion must be an integer between 0 and 100.'
  }
  return null
}

function toInput(form: StoryFormInput): StoryboardStoryInput {
  return {
    id: form.id.trim(),
    workstream: form.workstream,
    title: form.title.trim(),
    priority: form.priority,
    status: form.status,
    notes: form.notes ?? '',
    batch: form.batch,
    goal: form.goal?.trim() || null,
    scope: form.scope?.trim() || null,
    acceptanceCriteria: form.acceptanceCriteria?.trim() || null,
    dependencies: form.dependencies?.trim() || null,
    // Complete forces completion = 100 (enforced centrally).
    completion: form.status === 'Complete' ? 100 : form.completion,
    rollup: form.rollup,
    plannedStartAt: form.plannedStartAt?.trim() || null,
    actualStartAt: form.actualStartAt?.trim() || null,
    completedAt: form.completedAt?.trim() || null,
  }
}

function revalidateStoryBoard() {
  revalidatePath('/portal/storyboard')
}

export async function createStoryAction(
  form: StoryFormInput,
): Promise<StoryBoardWriteResult<StoryboardStory>> {
  const error = validateInput(form, true)
  if (error) return { ok: false, code: 'validation', message: error }

  try {
    const story = await createStoryboardStory(toInput(form))
    revalidateStoryBoard()
    return { ok: true, data: story }
  } catch (error) {
    return failure(error)
  }
}

export async function updateStoryAction(
  id: string,
  form: StoryFormInput,
): Promise<StoryBoardWriteResult<StoryboardStory>> {
  const error = validateInput(form, false)
  if (error) return { ok: false, code: 'validation', message: error }

  try {
    const story = await updateStoryboardStory(id, toInput(form))
    revalidateStoryBoard()
    return { ok: true, data: story }
  } catch (error) {
    return failure(error)
  }
}

export async function setStoryStatusAction(
  id: string,
  status: string,
): Promise<StoryBoardWriteResult<StoryboardStory>> {
  if (!STORY_STATUSES.includes(status as StoryStatus)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Status is required.',
    }
  }

  try {
    const story = await setStoryboardStatus(id, status)
    revalidateStoryBoard()
    return { ok: true, data: story }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------------------
// Execution run lifecycle (narrow agent interface)
// ---------------------------------------------------------------------------

export async function startStoryRunAction(
  storyId: string,
): Promise<StoryBoardWriteResult<{ run: StoryRun; story: StoryboardStory }>> {
  if (!storyId.trim()) {
    return { ok: false, code: 'validation', message: 'Story ID is required.' }
  }
  try {
    const result = await startStoryRun(storyId.trim())
    revalidateStoryBoard()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function finishStoryRunAction(
  runId: string,
  input: {
    resultStatus: string
    completion: number
    notes: string
    commitHash?: string | null
    testsSummary?: string | null
  },
): Promise<StoryBoardWriteResult<{ run: StoryRun; story: StoryboardStory }>> {
  if (!runId.trim()) {
    return { ok: false, code: 'validation', message: 'Run ID is required.' }
  }
  if (!RUN_RESULT_STATUSES.includes(input.resultStatus as RunResultStatus)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Result status must be one of the execution outcomes.',
    }
  }
  if (
    typeof input.completion !== 'number' ||
    !Number.isInteger(input.completion) ||
    input.completion < 0 ||
    input.completion > 100
  ) {
    return {
      ok: false,
      code: 'validation',
      message: 'Completion must be an integer between 0 and 100.',
    }
  }

  const runInput: FinishRunInput = {
    resultStatus: input.resultStatus,
    completion: input.completion,
    notes: input.notes ?? '',
    commitHash: input.commitHash?.trim() || null,
    testsSummary: input.testsSummary?.trim() || null,
  }

  try {
    const result = await finishStoryRun(runId.trim(), runInput)
    revalidateStoryBoard()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

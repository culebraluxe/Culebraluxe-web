'use server'

import { revalidatePath } from 'next/cache'
import { redirect } from 'next/navigation'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { completeTask, updateTask } from '@/db/portal-writes'
import { createTask } from '@/db/tasks'

// CATCH-UP — Task Workspace server actions (save / create / complete).
//
// These are the UI command layer that REUSES the canonical task write service
// (db/portal-writes.ts: updateTask / completeTask; db/tasks.ts: createTask). No
// parallel task mutation system is created here. Every action enforces the same
// portal.read boundary as the rest of the /portal surface and revalidates the
// Catch-Up route so the server-projected queue stays canonical.

export type TaskWriteState = {
  ok: boolean
  error?: string
  taskId?: string
  title?: string
  detail?: string | null
  dueAt?: string | null
  priority?: number
  workstream?: string | null
  category?: string | null
} | null

async function requireRead(): Promise<void> {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'portal.read',
  )
  if (!access.ok) redirect(access.redirectTo)
}

function parseDue(raw: string): string | null {
  const value = raw.trim()
  if (!value) return null
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

function readTaxonomy(formData: FormData): {
  workstream: string | null
  category: string | null
} {
  const workstream = String(formData.get('workstream') ?? '').trim().toUpperCase() || null
  const category = String(formData.get('category') ?? '').trim().toUpperCase() || null
  return { workstream, category }
}

export async function saveTaskAction(
  _prev: TaskWriteState,
  formData: FormData,
): Promise<TaskWriteState> {
  await requireRead()

  const taskId = String(formData.get('taskId') ?? '').trim()
  const title = String(formData.get('title') ?? '').trim()
  const detail = String(formData.get('detail') ?? '').trim() || null
  const dueAt = parseDue(String(formData.get('targetDate') ?? ''))
  const priorityRaw = Number(formData.get('priority'))
  const priority = Number.isFinite(priorityRaw) ? Math.floor(priorityRaw) : 0
  const { workstream, category } = readTaxonomy(formData)

  if (!taskId) return { ok: false, error: 'Missing task.' }
  if (!title) return { ok: false, error: 'Task title is required.' }
  if (!workstream) return { ok: false, error: 'Workstream is required.' }

  try {
    await updateTask(taskId, {
      title,
      detail,
      dueAt,
      priority,
      workstream,
      category,
    })
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Could not save the task.',
    }
  }

  revalidatePath('/portal/catch-up')
  return { ok: true, taskId, title, detail, dueAt, priority, workstream, category }
}

export async function createTaskAction(
  _prev: TaskWriteState,
  formData: FormData,
): Promise<TaskWriteState> {
  await requireRead()

  const title = String(formData.get('title') ?? '').trim()
  const detail = String(formData.get('detail') ?? '').trim() || null
  const dueAt = parseDue(String(formData.get('targetDate') ?? ''))
  const priorityRaw = Number(formData.get('priority'))
  const priority = Number.isFinite(priorityRaw) ? Math.floor(priorityRaw) : 1
  const { workstream, category } = readTaxonomy(formData)

  if (!title) return { ok: false, error: 'Task title is required.' }
  if (!workstream) return { ok: false, error: 'Workstream is required.' }

  try {
    const task = await createTask({
      title,
      detail: detail ?? undefined,
      dueAt: dueAt ?? undefined,
      priority,
      workstream,
      category,
    })
    revalidatePath('/portal/catch-up')
    return {
      ok: true,
      taskId: task.id,
      title,
      detail,
      dueAt,
      priority,
      workstream,
      category,
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Could not create the task.',
    }
  }
}

export async function completeTaskAction(
  _prev: TaskWriteState,
  formData: FormData,
): Promise<TaskWriteState> {
  await requireRead()

  const taskId = String(formData.get('taskId') ?? '').trim()
  if (!taskId) return { ok: false, error: 'Missing task.' }

  try {
    await completeTask(taskId)
  } catch (error) {
    return {
      ok: false,
      error:
        error instanceof Error
          ? error.message
          : 'Could not complete the task.',
    }
  }

  revalidatePath('/portal/catch-up')
  return { ok: true, taskId }
}

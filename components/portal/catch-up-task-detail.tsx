'use client'

import type { ReactNode } from 'react'

import { Panel } from '@/components/portal/panel'
import type { CatchUpTask } from '@/db/tasks'

// ---------------------------------------------------------------------------
// CATCH-UP — Task Workspace (middle pane of the three-pane Catch-Up layout).
//
// The selected task is the star. This is an "enriched Post-it note" — the task
// title dominates, vertical space is used generously, and only information that
// actually exists is shown (absent fields are OMITTED, never rendered as "-").
//
// It borrows the Client detail/card visual language (Panel compact + serif
// heading + uppercase micro-labels) so it feels related to the existing portal,
// not like a new admin form. Not an edit form in this pass — the operator WORKS
// the task here, but this slice is primarily layout/navigation.
// ---------------------------------------------------------------------------

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
        {label}
      </div>
      <div className="mt-1 break-words text-sm font-light leading-5 text-black/70">
        {children}
      </div>
    </div>
  )
}

function formatDue(iso: string | null): string | null {
  if (!iso) return null
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  })
}

function statusLabel(status: string): string {
  if (!status) return ''
  return status.charAt(0).toUpperCase() + status.slice(1)
}

export function CatchUpTaskDetail({ task }: { task: CatchUpTask | null }) {
  if (!task) {
    return (
      <Panel compact variant="soft" lifted heading="Task Detail" className="flex h-full min-h-0 min-w-0 flex-col">
        <p className="flex flex-1 items-center justify-center px-4 text-center text-sm font-light text-black/40">
          Select a task from the navigator.
        </p>
      </Panel>
    )
  }

  const workstream = (task.workstream ?? 'Workstream').toUpperCase()
  const category = (task.category ?? 'General').toUpperCase()
  const due = formatDue(task.dueAt)

  return (
    <Panel compact variant="soft" lifted heading="Task Detail" className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="min-w-0">
          <p className="text-[10px] font-light uppercase tracking-[0.22em] text-[var(--portal-panel-heading-muted)]">
            {workstream}
          </p>
          <h3 className="mt-1.5 font-serif text-2xl font-light leading-tight text-[var(--portal-navy)]">
            {task.title}
          </h3>
          <p className="mt-2 text-xs font-light text-black/50">
            {workstream} / {category}
          </p>
        </div>

        <div className="mt-6 grid content-start gap-4 border-t border-[var(--portal-panel-border)] pt-4 sm:grid-cols-2">
          {task.ownerName ? <Field label="Owner">{task.ownerName}</Field> : null}
          {task.status ? <Field label="Status">{statusLabel(task.status)}</Field> : null}
          {typeof task.priority === 'number' && task.priority > 0 ? (
            <Field label="Priority">{task.priority}</Field>
          ) : null}
          {due ? <Field label="Due">{due}</Field> : null}
          {task.personName ? <Field label="Person">{task.personName}</Field> : null}
          {task.propertyName ? (
            <Field label="Property">{task.propertyName}</Field>
          ) : null}
          {task.dealName ? <Field label="Deal">{task.dealName}</Field> : null}
        </div>

        {task.detail ? (
          <div className="mt-6 border-t border-[var(--portal-panel-border)] pt-4">
            <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
              Detail
            </div>
            <p className="mt-2 whitespace-pre-wrap text-[15px] font-light leading-7 text-black/70">
              {task.detail}
            </p>
          </div>
        ) : null}
      </div>
    </Panel>
  )
}

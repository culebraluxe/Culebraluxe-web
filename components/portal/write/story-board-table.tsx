'use client'

import { useState, useTransition } from 'react'

import {
  createStoryAction,
  setStoryStatusAction,
  updateStoryAction,
  type StoryFormInput,
} from '@/app/portal/storyboard/actions'
import {
  STORY_PRIORITIES,
  STORY_STATUSES,
  WORKSTREAMS,
  type StoryRecord,
} from '@/lib/storyboard-data'

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  return result as Result
}

const statusClasses: Record<string, string> = {
  Complete: 'bg-emerald-50 text-emerald-700',
  Operationalized: 'bg-emerald-50 text-emerald-700',
  'Minor remainder': 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]',
  'Read-side complete': 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]',
  'Readiness PASS': 'bg-amber-50 text-amber-700',
  Partial: 'bg-black/5 text-black/55',
  Planned: 'bg-black/5 text-black/55',
  Open: 'bg-black/5 text-black/55',
  Blocked: 'bg-red-50 text-red-700',
  Deferred: 'bg-black/5 text-black/40',
  'Hardware/content dependent': 'bg-black/5 text-black/55',
}

const priorityClasses: Record<string, string> = {
  Critical: 'bg-red-50 text-red-700',
  High: 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]',
  'High-ish': 'bg-[#c6a15b]/15 text-[#8a6d2f]',
  'Medium-High': 'bg-black/5 text-black/60',
  Medium: 'bg-black/5 text-black/50',
  Low: 'bg-black/5 text-black/40',
  Later: 'bg-black/5 text-black/40',
  'High-value polish': 'bg-[#c6a15b]/15 text-[#8a6d2f]',
}

const primaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40'

const secondaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40'

const ghostButton =
  'inline-flex items-center rounded-sm px-2 py-1 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[#8a4b2a] disabled:cursor-not-allowed disabled:opacity-40'

const fieldLabel =
  'text-[10px] font-light uppercase tracking-[0.18em] text-black/40'

const textInput =
  'mt-2 block w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 py-2.5 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]'

const selectInput =
  'mt-2 block min-h-11 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]'

function emptyForm(): StoryFormInput {
  return {
    id: '',
    workstream: WORKSTREAMS[0],
    title: '',
    priority: 'Medium',
    status: 'Open',
    notes: '',
    batch: null,
    goal: null,
    scope: null,
    acceptanceCriteria: null,
    dependencies: null,
  }
}

function StoryForm({
  initial,
  idEditable,
  submitLabel,
  isPending,
  onSubmit,
  onCancel,
}: {
  initial: StoryFormInput
  idEditable: boolean
  submitLabel: string
  isPending: boolean
  onSubmit: (form: StoryFormInput) => void
  onCancel: () => void
}) {
  const [form, setForm] = useState<StoryFormInput>(initial)
  const [localError, setLocalError] = useState<string | null>(null)

  function setField<K extends keyof StoryFormInput>(
    key: K,
    value: StoryFormInput[K],
  ) {
    setForm((current) => ({ ...current, [key]: value }))
  }

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setLocalError(null)
    if (idEditable && !form.id.trim()) {
      setLocalError('Story ID is required.')
      return
    }
    if (!form.title.trim()) {
      setLocalError('Story title is required.')
      return
    }
    onSubmit(form)
  }

  return (
    <form onSubmit={submit} className="space-y-4">
      <div className="grid gap-4 md:grid-cols-3">
        {idEditable ? (
          <label className="block">
            <span className={fieldLabel}>Story ID</span>
            <input
              type="text"
              value={form.id}
              onChange={(event) => setField('id', event.target.value)}
              placeholder="e.g. CRM-19, OPS-07, PX-27"
              className={textInput}
            />
            <span className="mt-1.5 block text-xs font-light text-black/40">
              Human-assigned; fixed after creation.
            </span>
          </label>
        ) : (
          <div>
            <span className={fieldLabel}>Story ID</span>
            <div className="mt-2 rounded-sm border border-[var(--portal-border)] bg-black/[0.03] px-3 py-2.5 font-mono text-sm font-light text-black/50">
              {form.id}
            </div>
          </div>
        )}

        <label className="block">
          <span className={fieldLabel}>Workstream</span>
          <select
            value={form.workstream}
            onChange={(event) => setField('workstream', event.target.value)}
            className={`${selectInput} w-full`}
          >
            {WORKSTREAMS.map((workstream) => (
              <option key={workstream} value={workstream}>
                {workstream}
              </option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className={fieldLabel}>Title</span>
          <input
            type="text"
            value={form.title}
            onChange={(event) => setField('title', event.target.value)}
            className={textInput}
          />
        </label>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <label className="block">
          <span className={fieldLabel}>Priority</span>
          <select
            value={form.priority}
            onChange={(event) => setField('priority', event.target.value)}
            className={`${selectInput} w-full`}
          >
            {STORY_PRIORITIES.map((priority) => (
              <option key={priority} value={priority}>
                {priority}
              </option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className={fieldLabel}>Status</span>
          <select
            value={form.status}
            onChange={(event) => setField('status', event.target.value)}
            className={`${selectInput} w-full`}
          >
            {STORY_STATUSES.map((status) => (
              <option key={status} value={status}>
                {status}
              </option>
            ))}
          </select>
        </label>
      </div>

      <label className="block">
        <span className={fieldLabel}>Notes</span>
        <textarea
          value={form.notes}
          onChange={(event) => setField('notes', event.target.value)}
          rows={2}
          className={`${textInput} resize-y`}
        />
      </label>


      <div className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-bg)]/60 p-4">
        <div className={fieldLabel}>Details (optional)</div>
        <div className="mt-3 grid gap-4 md:grid-cols-2">
          <label className="block">
            <span className={fieldLabel}>Batch</span>
            <input
              type="number"
              min={1}
              value={form.batch ?? ''}
              onChange={(event) =>
                setField(
                  'batch',
                  event.target.value === '' ? null : Number(event.target.value),
                )
              }
              className={textInput}
            />
          </label>

          <label className="block">
            <span className={fieldLabel}>Dependencies</span>
            <input
              type="text"
              value={form.dependencies ?? ''}
              onChange={(event) => setField('dependencies', event.target.value)}
              className={textInput}
            />
          </label>

          <label className="block">
            <span className={fieldLabel}>Goal</span>
            <textarea
              value={form.goal ?? ''}
              onChange={(event) => setField('goal', event.target.value)}
              rows={2}
              className={`${textInput} resize-y`}
            />
          </label>

          <label className="block">
            <span className={fieldLabel}>Scope</span>
            <textarea
              value={form.scope ?? ''}
              onChange={(event) => setField('scope', event.target.value)}
              rows={2}
              className={`${textInput} resize-y`}
            />
          </label>

          <label className="block md:col-span-2">
            <span className={fieldLabel}>Acceptance criteria</span>
            <textarea
              value={form.acceptanceCriteria ?? ''}
              onChange={(event) =>
                setField('acceptanceCriteria', event.target.value)
              }
              rows={2}
              className={`${textInput} resize-y`}
            />
          </label>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <button type="submit" disabled={isPending} className={primaryButton}>
          {submitLabel}
        </button>
        <button
          type="button"
          disabled={isPending}
          onClick={onCancel}
          className={secondaryButton}
        >
          Cancel
        </button>
        {localError && (
          <span className="text-xs font-light text-[#8a4b2a]">
            {localError}
          </span>
        )}
      </div>
    </form>
  )
}


function StoryRow({
  story,
  isEditing,
  isPending,
  onEdit,
  onCancelEdit,
  onSave,
  onChangeStatus,
}: {
  story: StoryRecord
  isEditing: boolean
  isPending: boolean
  onEdit: () => void
  onCancelEdit: () => void
  onSave: (form: StoryFormInput) => void
  onChangeStatus: (status: string) => void
}) {
  return (
    <>
      <tr className="border-b border-[var(--portal-border)]">
        <td className="px-6 py-4 align-top font-mono text-xs text-[var(--portal-navy)]">
          {story.id}
        </td>
        <td className="px-6 py-4 align-top">
          <div className="font-light leading-6 text-[var(--portal-navy)]">
            {story.title}
          </div>
          <button
            type="button"
            disabled={isPending}
            onClick={onEdit}
            className={ghostButton}
          >
            Edit
          </button>
        </td>
        <td className="px-6 py-4 align-top">
          <span
            className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-xs font-light ${priorityClasses[story.priority]}`}
          >
            {story.priority}
          </span>
        </td>
        <td className="px-6 py-4 align-top">
          <select
            value={story.status}
            disabled={isPending}
            onChange={(event) => onChangeStatus(event.target.value)}
            className={`inline-block min-h-8 cursor-pointer whitespace-nowrap rounded-full px-3 text-xs font-light outline-none ${statusClasses[story.status]}`}
          >
            {STORY_STATUSES.map((status) => (
              <option key={status} value={status}>
                {status}
              </option>
            ))}
          </select>
        </td>
        <td className="px-6 py-4 align-top text-sm font-light leading-6 text-black/55">
          {story.notes}
        </td>
      </tr>

      {isEditing && (
        <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-bg)]/40">
          <td colSpan={5} className="px-6 py-5">
            <StoryForm
              key={story.id}
              initial={{
                id: story.id,
                workstream: story.workstream,
                title: story.title,
                priority: story.priority,
                status: story.status,
                notes: story.notes,
                batch: story.batch,
                goal: story.goal,
                scope: story.scope,
                acceptanceCriteria: story.acceptanceCriteria,
                dependencies: story.dependencies,
              }}
              idEditable={false}
              submitLabel="Save changes"
              isPending={isPending}
              onSubmit={onSave}
              onCancel={onCancelEdit}
            />
          </td>
        </tr>
      )}
    </>
  )
}


export function StoryBoardTable({ stories }: { stories: StoryRecord[] }) {
  const [isPending, startTransition] = useTransition()
  const [creating, setCreating] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )

  function createStory(form: StoryFormInput) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await createStoryAction(form))
      if (result.ok) {
        setCreating(false)
        setMessage({ ok: true, text: `Story ${form.id} created.` })
      } else {
        setMessage({
          ok: false,
          text: result.message ?? 'Could not create story.',
        })
      }
    })
  }

  function saveStory(id: string, form: StoryFormInput) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await updateStoryAction(id, form))
      if (result.ok) {
        setEditingId(null)
        setMessage({ ok: true, text: `Story ${id} updated.` })
      } else {
        setMessage({
          ok: false,
          text: result.message ?? 'Could not update story.',
        })
      }
    })
  }

  function changeStatus(id: string, status: string) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await setStoryStatusAction(id, status))
      if (result.ok) {
        setMessage({ ok: true, text: `Story ${id} status updated.` })
      } else {
        setMessage({
          ok: false,
          text: result.message ?? 'Could not update status.',
        })
      }
    })
  }

  const editingStory =
    editingId === null ? null : stories.find((s) => s.id === editingId) ?? null

  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="border-b border-[var(--portal-border)] px-6 py-6">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 className="font-serif text-2xl font-light">Story Backlog</h2>
            <p className="mt-1 text-sm font-light text-black/50">
              The existing human-authored backlog, grouped by workstream. Create,
              edit and change status here; changes are persisted to Neon.
            </p>
          </div>
          <div className="flex items-center gap-4">
            {message && (
              <span
                className={`max-w-xs text-xs font-light ${
                  message.ok ? 'text-black/50' : 'text-[#8a4b2a]'
                }`}
              >
                {message.text}
              </span>
            )}
            <span className="text-xs font-light uppercase tracking-[0.18em] text-black/40">
              {stories.length} stories
            </span>
            <button
              type="button"
              disabled={isPending || creating}
              onClick={() => {
                setCreating(true)
                setEditingId(null)
                setMessage(null)
              }}
              className={primaryButton}
            >
              New Story
            </button>
          </div>
        </div>

        {creating && (
          <div className="mt-6 rounded-sm border border-[var(--portal-border)] bg-[var(--portal-bg)]/40 p-5">
            <div className="mb-4 flex items-baseline justify-between gap-4">
              <h3 className="font-serif text-xl font-light">New Story</h3>
              <span className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                Human-assigned ID
              </span>
            </div>
            <StoryForm
              key="create"
              initial={emptyForm()}
              idEditable
              submitLabel="Create story"
              isPending={isPending}
              onSubmit={createStory}
              onCancel={() => {
                setCreating(false)
                setMessage(null)
              }}
            />
          </div>
        )}
      </div>


      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-left text-sm">
          <thead>
            <tr className="border-b border-[var(--portal-border)]">
              <th className="w-24 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                ID
              </th>
              <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Story
              </th>
              <th className="w-40 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Priority
              </th>
              <th className="w-48 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Status
              </th>
              <th className="min-w-72 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Notes
              </th>
            </tr>
          </thead>

          {WORKSTREAMS.map((workstream) => {
            const rows = stories.filter((s) => s.workstream === workstream)

            return (
              <tbody key={workstream}>
                <tr className="bg-[var(--portal-blue-pale)]/50">
                  <td
                    colSpan={5}
                    className="px-6 py-3 text-[10px] font-light uppercase tracking-[0.24em] text-[var(--portal-navy)]"
                  >
                    {workstream}
                    <span className="ml-3 text-black/40">
                      {rows.length} story{rows.length === 1 ? '' : 's'}
                    </span>
                  </td>
                </tr>

                {rows.length === 0 ? (
                  <tr className="border-b border-[var(--portal-border)]">
                    <td
                      colSpan={5}
                      className="px-6 py-6 text-sm font-light italic text-black/40"
                    >
                      No stories tracked under this workstream yet.
                    </td>
                  </tr>
                ) : (
                  rows.map((story) => (
                    <StoryRow
                      key={story.id}
                      story={story}
                      isEditing={editingStory?.id === story.id}
                      isPending={isPending}
                      onEdit={() => {
                        setEditingId(story.id)
                        setCreating(false)
                        setMessage(null)
                      }}
                      onCancelEdit={() => {
                        setEditingId(null)
                        setMessage(null)
                      }}
                      onSave={(form) => saveStory(story.id, form)}
                      onChangeStatus={(status) => changeStatus(story.id, status)}
                    />
                  ))
                )}
              </tbody>
            )
          })}
        </table>
      </div>
    </section>
  )
}

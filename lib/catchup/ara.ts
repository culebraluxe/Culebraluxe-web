// ---------------------------------------------------------------------------
// ARA — CATCH-UP command interpreter (pure, deterministic).
//
// Maps the deliberately tiny Catch-Up command surface onto the canonical task
// mutation seams. Two supported actions only:
//   - EDIT the currently selected task
//   - CREATE a new task
//
// Calendar / task→calendar drag / scheduling is intentionally OUT OF SCOPE for
// this pass and is reported as unsupported — never faked.
//
// The interpreter is pure (no DB, no React, no server actions): it translates a
// natural-language instruction plus a runtime context into one of:
//   - { kind: 'edit' }   — full merged update fields for the selected task
//   - { kind: 'create' } — full creation fields for a new task
//   - { kind: 'ask' }    — one concise clarification question
//   - { kind: 'unsupported' } — a clearly out-of-scope request
//
// The caller (catch-up.tsx) is responsible for invoking the EXISTING canonical
// seams (saveTaskAction / createTaskAction) with these fields — never another
// task mutation path, never a direct database write.
//
// The capabilities guide that mirrors this behaviour lives at
// docs/ara/catch-up-capabilities.md.
// ---------------------------------------------------------------------------

import { CATCHUP_CATEGORIES } from './task-taxonomy'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type AraSelectedTask = {
  id: string
  title: string
  detail: string | null
  dueAt: string | null
  workstream: string | null
  category: string | null
  priority: number
}

/** The small runtime context given to Ara (never the whole task queue). */
export type AraRuntimeContext = {
  currentWorkstream: string | null
  selectedTask: AraSelectedTask | null
}

/** Full, valid update payload for the canonical updateTask seam (edit). */
export type AraEditFields = {
  title: string
  detail: string | null
  dueAt: string | null
  priority: number
  workstream: string
  category: string | null
}

/** Full, valid creation payload for the canonical createTask seam (create). */
export type AraCreateFields = {
  title: string
  detail: string | null
  dueAt: string | null
  priority: number
  workstream: string
  category: string | null
}

export type AraResult =
  | { kind: 'edit'; taskId: string; fields: AraEditFields; message: string }
  | { kind: 'create'; fields: AraCreateFields; message: string }
  | { kind: 'ask'; question: string }
  | { kind: 'unsupported'; message: string }

// ---------------------------------------------------------------------------
// Bounded vocabulary
// ---------------------------------------------------------------------------

const WORKSTREAMS = Object.keys(CATCHUP_CATEGORIES) // CLIENT / CORE / OPPS / SUPPORT / TECH
const WORKSTREAM_RE = WORKSTREAMS.join('|')

// Existing Catch-Up priority convention (never surfaced as numbers to the user).
const PRIORITY_VALUE: Record<string, number> = { low: 0, medium: 1, high: 2 }
const DEFAULT_CREATE_PRIORITY = 1 // MEDIUM — matches the canonical create default

const MONTHS: Record<string, number> = {
  jan: 1, feb: 2, mar: 3, apr: 4, may: 5, jun: 6,
  jul: 7, aug: 8, sep: 9, oct: 10, nov: 11, dec: 12,
}
const DAYS: Record<string, number> = {
  sunday: 0, monday: 1, tuesday: 2, wednesday: 3,
  thursday: 4, friday: 5, saturday: 6,
}

const cap = (s: string) => (s ? s.charAt(0).toUpperCase() + s.slice(1) : s)

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/** A date at the given day-offset, pinned to 09:00 local, as an ISO string. */
function atLocalHour(now: Date, dayOffset: number, hour: number): string {
  const d = new Date(now)
  d.setDate(d.getDate() + dayOffset)
  d.setHours(hour, 0, 0, 0)
  return d.toISOString()
}

/**
 * Extract a human date phrase from arbitrary text ("tomorrow", "Friday",
 * "August 30", "8/30", "2026-08-30") and return its ISO value plus the matched
 * phrase. Returns nulls when no date phrase is present.
 */
export function extractDatePhrase(
  text: string,
  now: Date = new Date(),
): { iso: string | null; phrase: string | null } {
  const lower = text.toLowerCase()

  const iso = lower.match(/^(\d{4})-(\d{1,2})-(\d{1,2})$/)
  if (iso) {
    return {
      iso: atLocalHour(new Date(Number(iso[1]), Number(iso[2]) - 1, Number(iso[3])), 0, 9),
      phrase: iso[0],
    }
  }

  if (/\btomorrow\b/.test(lower)) return { iso: atLocalHour(now, 1, 9), phrase: 'tomorrow' }
  if (/\btoday\b/.test(lower)) return { iso: atLocalHour(now, 0, 9), phrase: 'today' }

  let m = lower.match(/\bnext\s+(sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b/)
  if (m) {
    const delta = (DAYS[m[1]] - now.getDay() + 7) % 7
    return { iso: atLocalHour(now, (delta === 0 ? 7 : delta) + 7, 9), phrase: m[0] }
  }

  m = lower.match(/\b(sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b/)
  if (m) {
    const delta = (DAYS[m[1]] - now.getDay() + 7) % 7
    return { iso: atLocalHour(now, delta === 0 ? 7 : delta, 9), phrase: m[0] }
  }

  m = lower.match(/\b([a-z]{3,9})\s+(\d{1,2})\b/)
  if (m && MONTHS[m[1].slice(0, 3)]) {
    const month = MONTHS[m[1].slice(0, 3)]
    const day = Number(m[2])
    const year = now.getMonth() + 1 > month ? now.getFullYear() + 1 : now.getFullYear()
    return { iso: atLocalHour(new Date(year, month - 1, day), 0, 9), phrase: m[0] }
  }

  m = lower.match(/\b(\d{1,2})\/(\d{1,2})(?:\/(\d{2,4}))?\b/)
  if (m) {
    const month = Number(m[1])
    const day = Number(m[2])
    const year = m[3]
      ? Number(m[3])
      : now.getMonth() + 1 > month
        ? now.getFullYear() + 1
        : now.getFullYear()
    return { iso: atLocalHour(new Date(year, month - 1, day), 0, 9), phrase: m[0] }
  }

  return { iso: null, phrase: null }
}

// ---------------------------------------------------------------------------
// Field parsers
// ---------------------------------------------------------------------------

/** Priority only when the instruction explicitly names a level + "priority". */
function extractPriority(text: string): number | null {
  const phrase = text.match(/\b(low|medium|high)\s+priority\b/i)
  if (phrase) return PRIORITY_VALUE[phrase[1].toLowerCase()]
  const order = text.match(/\bpriority\s+to\s+(low|medium|high)\b/i)
  if (order) return PRIORITY_VALUE[order[1].toLowerCase()]
  return null
}

type TaxonomyResult =
  | { ok: true; workstream: string; category: string | null }
  | { ok: false; workstream: string; categoryLabel: string }

/**
 * Extract an explicit Workstream / Category assignment for an EDIT instruction.
 * Recognises:
 *   "workstream to CORE and category to MARKETING"
 *   "workstream to CORE"
 *   "move this to CORE Marketing" / "move to CORE"
 * Returns null when no explicit taxonomy is present. An explicitly invalid
 * category returns { ok: false } so the caller refuses to persist and asks —
 * invalid taxonomy is never silently stored.
 */
function parseTaxonomy(prompt: string): TaxonomyResult | null {
  const both = prompt.match(
    new RegExp(`workstream\\s+to\\s+(${WORKSTREAM_RE})\\s+and\\s+category\\s+to\\s+([A-Za-z_]+)`, 'i'),
  )
  if (both) {
    const workstream = both[1].toUpperCase()
    const cand = both[2].toUpperCase()
    if (CATCHUP_CATEGORIES[workstream]?.includes(cand)) {
      return { ok: true, workstream, category: cand }
    }
    return { ok: false, workstream, categoryLabel: both[2] }
  }

  const wsOnly = prompt.match(new RegExp(`workstream\\s+to\\s+(${WORKSTREAM_RE})`, 'i'))
  if (wsOnly) return { ok: true, workstream: wsOnly[1].toUpperCase(), category: null }

  const move = prompt.match(
    new RegExp(`move(?:\\s+(?:this|it|the\\s+(?:selected\\s+)?task))?\\s+to\\s+(${WORKSTREAM_RE})(?:\\s+([A-Za-z_]+))?`, 'i'),
  )
  if (move) {
    const workstream = move[1].toUpperCase()
    if (move[2]) {
      const cand = move[2].toUpperCase()
      return CATCHUP_CATEGORIES[workstream]?.includes(cand)
        ? { ok: true, workstream, category: cand }
        : { ok: false, workstream, categoryLabel: move[2] }
    }
    return { ok: true, workstream, category: null }
  }

  return null
}



/** Parse an Edit instruction into merged update fields (or ask). */

/** Parse a Create instruction into its fields (or ask). */
function parseCreate(
  prompt: string,
  ctx: AraRuntimeContext,
): Extract<AraResult, { kind: 'create' }> | Extract<AraResult, { kind: 'ask' }> {
  let text = prompt.trim().replace(/[.]+$/, '')

  // Strip the leading create verb + article ("create a", "add a", "make a new").
  text = text.replace(/^\s*(?:create|add|make|new)\b\s+(?:a|an|the)?\s*/i, '')

  let priority = DEFAULT_CREATE_PRIORITY
  let workstream: string | null = null
  let category: string | null = null

  const prio = text.match(/\b(low|medium|high)\s+priority\b/i)
  if (prio) {
    priority = PRIORITY_VALUE[prio[1].toLowerCase()]
    text = text.replace(/\b(low|medium|high)\s+priority\b/i, ' ')
  }

  // Taxonomy: "<WS> [<CAT>] task" (e.g. "CORE Marketing task", "OPPS task").
  const tax = text.match(
    new RegExp(`\\b(${WORKSTREAM_RE})(?:\\s+([A-Za-z_]+))?\\s+task\\b`, 'i'),
  )
  if (tax) {
    const ws = tax[1].toUpperCase()
    if (tax[2]) {
      const cand = tax[2].toUpperCase()
      if (CATCHUP_CATEGORIES[ws]?.includes(cand)) {
        category = cand
      } else {
        return {
          kind: 'ask',
          question: `“${ws}” doesn't have a “${tax[2]}” category — which did you mean?`,
        }
      }
    }
    workstream = ws
    text = text.replace(
      new RegExp(`\\b(${WORKSTREAM_RE})(?:\\s+[A-Za-z_]+)?\\s+task\\b`, 'i'),
      ' ',
    )
  }

  text = text.replace(/\btask\b/i, ' ')
  text = text.replace(/^\s*to\s+/i, '')

  const title = text.replace(/\s+/g, ' ').trim()
  if (!title) {
    return { kind: 'ask', question: 'What should the new task be?' }
  }

  const effectiveWorkstream = workstream ?? ctx.currentWorkstream
  if (!effectiveWorkstream) {
    return { kind: 'ask', question: 'Which workstream should this task belong to?' }
  }

  const due = extractDatePhrase(title)

  const fields: AraCreateFields = {
    title,
    detail: null,
    dueAt: due.iso,
    priority,
    workstream: effectiveWorkstream,
    category,
  }
  return { kind: 'create', fields, message: `Created: “${title}”.` }
}

function parseEdit(
  prompt: string,
  ctx: AraRuntimeContext,
): Extract<AraResult, { kind: 'edit' }> | Extract<AraResult, { kind: 'ask' }> {
  const sel = ctx.selectedTask
  if (!sel) {
    return { kind: 'ask', question: 'Select a task first, then tell me what to change.' }
  }

  // Merge base = current selected-task values; apply ONLY requested changes.
  let title = sel.title
  let detail = sel.detail ?? null
  let dueAt = sel.dueAt ?? null
  let priority = sel.priority
  let workstream = sel.workstream ?? ctx.currentWorkstream ?? ''
  let category = sel.category ?? null

  let changed = false
  const changedFields: string[] = []

  const titleMatch = prompt.match(/(?:change|set|update)\s+the\s+title\s+to\s+(.+)/i)
  if (titleMatch) {
    title = titleMatch[1].trim().replace(/[.]+$/, '')
    if (!title) return { kind: 'ask', question: 'What should the new title be?' }
    changed = true
    changedFields.push('title')
  }

  const prio = extractPriority(prompt)
  if (prio !== null) {
    priority = prio
    changed = true
    changedFields.push('priority')
  }

  const dateMatch = prompt.match(/(?:target date|due date|deadline)\s+to\s+(.+)/i)
  if (dateMatch) {
    const date = extractDatePhrase(dateMatch[1])
    if (date?.iso) {
      dueAt = date.iso
      changed = true
      changedFields.push('target date')
    } else {
      return {
        kind: 'ask',
        question: `I couldn't read the target date in “${dateMatch[1].trim()}”. Try “tomorrow” or a specific date.`,
      }
    }
  }

  const noteMatch = prompt.match(
    /(?:add(?:ed)?\s+(?:(?:a|this)\s+)?note|add\s+note)\s*(?::|that)?\s*(.+)/i,
  )
  if (noteMatch) {
    const note = noteMatch[1].trim().replace(/[.]+$/, '')
    if (note) {
      detail = detail ? `${detail}\n${note}` : note
      changed = true
      changedFields.push('note')
    }
  }

  const taxonomy = parseTaxonomy(prompt)
  if (taxonomy) {
    if (!taxonomy.ok) {
      return {
        kind: 'ask',
        question: `“${taxonomy.categoryLabel}” isn't a category in ${taxonomy.workstream} — which did you mean?`,
      }
    }
    workstream = taxonomy.workstream
    if (taxonomy.category !== null) category = taxonomy.category
    changed = true
    changedFields.push('workstream/category')
  }

  if (!changed) {
    return {
      kind: 'ask',
      question:
        'I didn’t catch what to change. Try “Make this high priority”, “Change the target date to tomorrow”, or “Add a note…”',
    }
  }

  const fields: AraEditFields = { title, detail, dueAt, priority, workstream, category }
  return {
    kind: 'edit',
    taskId: sel.id,
    fields,
    message: `${changedFields.map(cap).join(', ')} updated.`,
  }
}

// ---------------------------------------------------------------------------
// Intent detection
// ---------------------------------------------------------------------------

function detectCreateIntent(lower: string): boolean {
  // Anything that explicitly refers to the selected task is an edit.
  if (/\b(this|it|the selected task)\b/.test(lower)) return false
  const isNote = /\b(add(?:ed)?\s+(?:(?:a|this)\s+)?note|add\s+note)\b/.test(lower)
  if (isNote) return false
  const createVerb = /\b(create|add|make|new)\b/.test(lower)
  if (!createVerb) return false
  return (
    /\btask\b/.test(lower) ||
    /\b(create|make|add)\s+a\s+(?:new\s+)?task\b/.test(lower) ||
    /^create\b/.test(lower)
  )
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/**
 * Interpret a Catch-Up Ara instruction against the runtime context.
 * Pure function — never mutates, never writes, never guesses another task.
 */
export function interpretAraCommand(
  prompt: string,
  ctx: AraRuntimeContext,
): AraResult {
  const original = prompt.trim()
  const lower = original.toLowerCase()
  if (!lower) {
    return { kind: 'ask', question: 'Tell me what you would like me to do.' }
  }

  // Calendar / task→calendar drag / scheduling is a later, separate capability.
  if (/\b(schedule|scheduling|drag|dragging)\b/.test(lower) || /\bcalendar\b/.test(lower)) {
    return {
      kind: 'unsupported',
      message:
        'Dragging tasks onto the calendar is not available yet — I can edit or create a task for now.',
    }
  }

  return detectCreateIntent(lower) ? parseCreate(original, ctx) : parseEdit(original, ctx)
}


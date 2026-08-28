import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  extractDatePhrase,
  interpretAraCommand,
  type AraRuntimeContext,
} from '../../lib/catchup/ara'

// ---------------------------------------------------------------------------
// ARA CATCH-UP — capability guide + real command interpretation.
//
// The interpreter is pure: it maps a tiny instruction + runtime context to an
// EDIT (merged against the selected task) or CREATE (new canonical task), or a
// concise ASK, or UNSUPPORTED for the out-of-scope calendar path. It never
// writes. The integration to the canonical seams (saveTaskAction /
// createTaskAction) lives in catch-up.tsx and is exercised by the DEV smoke.
// ---------------------------------------------------------------------------

// Fixed "today" = 2026-08-27 12:00 local for deterministic date assertions.
const NOW = new Date(2026, 7, 27, 12, 0, 0)
const tomorrowIso = extractDatePhrase('tomorrow', NOW).iso

const selected = (over: Partial<AraRuntimeContext['selectedTask']> = {}) => ({
  id: 'task-1',
  title: 'Call Maria',
  detail: null,
  dueAt: null,
  workstream: 'CLIENT',
  category: 'FOLLOWUP',
  priority: 1,
  ...over,
})

const ctx = (
  selectedTask: AraRuntimeContext['selectedTask'],
  currentWorkstream: string | null = 'CLIENT',
): AraRuntimeContext => ({ currentWorkstream, selectedTask })

// --- EDIT: make this high priority ------------------------------------------
test('edit: "Make this high priority" changes only priority', () => {
  const r = interpretAraCommand('Make this high priority.', ctx(selected()))
  assert.equal(r.kind, 'edit')
  if (r.kind !== 'edit') return
  assert.equal(r.fields.priority, 2) // HIGH
  assert.equal(r.fields.title, 'Call Maria') // untouched
  assert.equal(r.fields.detail, null) // untouched
  assert.equal(r.fields.dueAt, null) // untouched
  assert.equal(r.fields.workstream, 'CLIENT') // untouched
  assert.equal(r.fields.category, 'FOLLOWUP') // untouched
})

// --- EDIT: change target date to tomorrow ------------------------------------
test('edit: "Change the target date to tomorrow" sets dueAt only', () => {
  const r = interpretAraCommand('Change the target date to tomorrow.', ctx(selected()), NOW)
  assert.equal(r.kind, 'edit')
  if (r.kind !== 'edit') return
  assert.equal(r.fields.dueAt, tomorrowIso)
  assert.equal(r.fields.priority, 1) // untouched
  assert.equal(r.fields.title, 'Call Maria') // untouched
})

// --- EDIT: change title --------------------------------------------------------
test('edit: "Change the title to Call attorney" changes title only', () => {
  const r = interpretAraCommand('Change the title to Call attorney.', ctx(selected()))
  assert.equal(r.kind, 'edit')
  if (r.kind !== 'edit') return
  assert.equal(r.fields.title, 'Call attorney')
  assert.equal(r.fields.priority, 1) // untouched
  assert.equal(r.fields.dueAt, null) // untouched
  assert.equal(r.fields.workstream, 'CLIENT') // untouched
})

// --- EDIT: add a note -----------------------------------------------------------
test('edit: "Add a note" appends to detail without touching other fields', () => {
  const r = interpretAraCommand('Add a note that the seller called this morning.', ctx(selected()))
  assert.equal(r.kind, 'edit')
  if (r.kind !== 'edit') return
  assert.equal(r.fields.detail, 'the seller called this morning')
  assert.equal(r.fields.priority, 1) // untouched
  assert.equal(r.fields.title, 'Call Maria') // untouched
  assert.equal(r.fields.dueAt, null) // untouched
})

// --- EDIT: move to CORE Marketing -------------------------------------------------
test('edit: "Move this to CORE Marketing" sets workstream + category', () => {
  const r = interpretAraCommand('Move this to CORE Marketing.', ctx(selected()))
  assert.equal(r.kind, 'edit')
  if (r.kind !== 'edit') return
  assert.equal(r.fields.workstream, 'CORE')
  assert.equal(r.fields.category, 'MARKETING')
  assert.equal(r.fields.priority, 1) // untouched
  assert.equal(r.fields.title, 'Call Maria') // untouched
})

// --- EDIT: workstream to CORE and category to MARKETING ----------------------------
test('edit: "Change the workstream to CORE and category to MARKETING" works', () => {
  const r = interpretAraCommand(
    'Change the workstream to CORE and category to MARKETING.',
    ctx(selected()),
  )
  assert.equal(r.kind, 'edit')
  if (r.kind !== 'edit') return
  assert.equal(r.fields.workstream, 'CORE')
  assert.equal(r.fields.category, 'MARKETING')
})

// --- EDIT failure: no selected task ---------------------------------------------
test('edit: no selected task asks to select one', () => {
  const r = interpretAraCommand('Make this high priority.', ctx(null))
  assert.equal(r.kind, 'ask')
})

// --- EDIT failure: ambiguous instruction ------------------------------------------
test('edit: ambiguous instruction asks one concise question', () => {
  const r = interpretAraCommand('Who should I follow up with?', ctx(selected()))
  assert.equal(r.kind, 'ask')
})

// --- EDIT failure: invalid taxonomy not silently persisted -------------------------
test('edit: invalid category is not silently persisted and asks', () => {
  const r = interpretAraCommand('Move this to CORE WEIRDSTUFF.', ctx(selected()))
  assert.equal(r.kind, 'ask')
})

// --- CREATE: call Lisa tomorrow -----------------------------------------------------
test('create: "Create a task to call Lisa tomorrow" uses current workstream + date', () => {
  const r = interpretAraCommand(
    'Create a task to call Lisa tomorrow.',
    ctx(null, 'CORE'),
    NOW,
  )
  assert.equal(r.kind, 'create')
  if (r.kind !== 'create') return
  assert.equal(r.fields.title, 'call Lisa tomorrow')
  assert.equal(r.fields.workstream, 'CORE') // current workstream default
  assert.equal(r.fields.dueAt, tomorrowIso)
  assert.equal(r.fields.priority, 1) // MEDIUM default
  assert.equal(r.fields.category, null)
})

// --- CREATE: high priority task -------------------------------------------------------
test('create: "Add a high priority task" sets priority HIGH', () => {
  const r = interpretAraCommand(
    'Add a high priority task to review Casa Luar photos.',
    ctx(null, 'CORE'),
  )
  assert.equal(r.kind, 'create')
  if (r.kind !== 'create') return
  assert.equal(r.fields.priority, 2) // HIGH
  assert.equal(r.fields.title, 'review Casa Luar photos')
  assert.equal(r.fields.workstream, 'CORE')
})

// --- CREATE: CORE Marketing task ---------------------------------------------------------
test('create: "Create a CORE Marketing task" persists taxonomy', () => {
  const r = interpretAraCommand(
    'Create a CORE Marketing task to update the Casa Luar brochure.',
    ctx(null, 'CORE'),
  )
  assert.equal(r.kind, 'create')
  if (r.kind !== 'create') return
  assert.equal(r.fields.workstream, 'CORE')
  assert.equal(r.fields.category, 'MARKETING')
  assert.equal(r.fields.title, 'update the Casa Luar brochure')
})

// --- CREATE failure: no workstream determinable --------------------------------------------
test('create: no workstream determinable asks which workstream', () => {
  const r = interpretAraCommand('Create a task to call John tomorrow.', ctx(null, null))
  assert.equal(r.kind, 'ask')
})

// --- CREATE failure: invalid taxonomy ---------------------------------------------------------
test('create: invalid category is not persisted and asks', () => {
  const r = interpretAraCommand(
    'Create a CORE WEIRDSTUFF task to update the brochure.',
    ctx(null, 'CORE'),
  )
  assert.equal(r.kind, 'ask')
})

// --- Calendar is out of scope for this pass ------------------------------------------------
test('calendar / scheduling is reported as unsupported, never faked', () => {
  const r = interpretAraCommand('Schedule this for Friday at 10.', ctx(selected()))
  assert.equal(r.kind, 'unsupported')
  const r2 = interpretAraCommand('Drag this task onto the calendar.', ctx(selected()))
  assert.equal(r2.kind, 'unsupported')
})

// --- extractDatePhrase -------------------------------------------------------------
test('extractDatePhrase: tomorrow / day-of-week / month-day / slash', () => {
  assert.equal(extractDatePhrase('tomorrow', NOW).iso, tomorrowIso)
  // 2026-08-27 is a Thursday; the coming Friday is 2026-08-28 (= tomorrow).
  const friday = extractDatePhrase('Friday', NOW)
  assert.equal(friday.phrase, 'friday')
  assert.equal(friday.iso, tomorrowIso)
  const aug30 = extractDatePhrase('August 30', NOW)
  assert.equal(aug30.phrase, 'august 30')
  const slash = extractDatePhrase('8/30', NOW)
  assert.equal(slash.iso, aug30.iso)
  assert.equal(extractDatePhrase('no date here', NOW).iso, null)
})


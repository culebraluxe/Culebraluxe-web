import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  applyLessonOccurrence,
  lessonFingerprint,
  lessonsForContext,
  type ForgeLesson,
  type LessonOccurrence,
} from '../forge/forge-lessons'

// ENG-FORGE-HARDEN-04 — recurring failures become durable, reviewable LESSONS;
// lessons are read-only context and can never mutate orchestration state.

function occ(over: Partial<LessonOccurrence>): LessonOccurrence {
  return {
    failureClass: 'MISSING_GUARDRAIL',
    affectedStage: 'LEAD_PRE -> SMITH',
    suggestedImprovement: 'validate execution-contract fields before Smith launch',
    affectedCapability: 'smith-launch',
    storyId: 'S1',
    runId: 'r1',
    atIso: '2026-01-01T00:00:00Z',
    ...over,
  }
}

test('eligible (systemic) failure -> a lesson is created with provenance', () => {
  const { store, lesson } = applyLessonOccurrence([], occ({ storyId: 'S1' }))
  assert.ok(lesson, 'lesson created')
  assert.equal(lesson!.failureClass, 'MISSING_GUARDRAIL')
  assert.equal(lesson!.recurring, false)
  assert.equal(lesson!.origin[0].storyId, 'S1')
  assert.equal(store.length, 1)
})

test('repeated equivalent failure -> recurrence is visible', () => {
  let store: ForgeLesson[] = []
  store = applyLessonOccurrence(store, occ({ storyId: 'S1', runId: 'r1' })).store
  store = applyLessonOccurrence(store, occ({ storyId: 'S2', runId: 'r2' })).store
  const lesson = store[0]
  assert.equal(lesson.recurrenceCount, 2)
  assert.equal(lesson.recurring, true)
  assert.equal(lesson.origin.length, 2, 'provenance accumulates across runs')
})

test('unrelated fingerprints remain separate lessons', () => {
  let store: ForgeLesson[] = []
  store = applyLessonOccurrence(store, occ({ affectedStage: 'SMITH -> QA' })).store
  store = applyLessonOccurrence(store, occ({ affectedStage: 'LEAD_PRE -> SMITH' })).store
  assert.equal(store.length, 2)
  assert.notEqual(store[0].fingerprint, store[1].fingerprint)
})

test('one-off ordinary implementation defect does NOT create an architectural lesson', () => {
  const before = []
  const { store, lesson } = applyLessonOccurrence(before, occ({ failureClass: 'BAD_IMPLEMENTATION' }))
  assert.equal(lesson, null, 'no noisy architectural lesson for a one-off defect')
  assert.equal(store.length, 0)
})

test('lessons are read-only context: they never mutate the caller store object', () => {
  const seed: ForgeLesson[] = []
  applyLessonOccurrence(seed, occ())
  assert.equal(seed.length, 0, 'the passed store array is not mutated')
})

test('future Scout/Architect context can retrieve lessons by class', () => {
  const { store } = applyLessonOccurrence([], occ({ failureClass: 'MISSING_GUARDRAIL' }))
  const byClass = lessonsForContext(store, 'MISSING_GUARDRAIL')
  assert.equal(byClass.length, 1)
  assert.equal(lessonsForContext(store, 'DEPLOYMENT_FAILURE').length, 0)
})

test('fingerprint is stable across occurrences', () => {
  const a = lessonFingerprint({ failureClass: 'MISSING_GUARDRAIL', affectedStage: 'LEAD_PRE -> SMITH' })
  const b = lessonFingerprint({ failureClass: 'MISSING_GUARDRAIL', affectedStage: 'LEAD_PRE -> SMITH' })
  assert.equal(a, b)
})

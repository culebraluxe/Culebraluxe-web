import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import {
  buildContextLessonDirective,
  FileContextLessonStore,
  lessonsForArea,
  MemoryContextLessonStore,
  type ForgeContextLesson,
} from '../forge/forge-context-lessons'

const lesson = (over: Partial<ForgeContextLesson> = {}): ForgeContextLesson => ({
  id: 'x1',
  area: ['forge', 'smith_split_work'],
  missingFact: 'split lanes inherit the Lead execution plan; never re-derive scope',
  source: 'lead_pre',
  createdAt: '2026-09-09T00:00:00.000Z',
  ...over,
})

test('context lessons: memory store adds, lists and removes', () => {
  const store = new MemoryContextLessonStore()
  assert.deepEqual(store.list(), [])
  const added = store.add({ area: ['smith'], missingFact: 'M', source: 's' })
  assert.equal(store.list().length, 1)
  assert.ok(added.id)
  store.remove(added.id)
  assert.deepEqual(store.list(), [])
})

test('context lessons: area matching is case-insensitive and substring-based', () => {
  const lessons = [lesson(), lesson({ id: 'x2', area: ['portal'] })]
  assert.equal(lessonsForArea(lessons, ['FORGE']).length, 1)
  assert.equal(lessonsForArea(lessons, ['smith_split']).length, 1)
  assert.equal(lessonsForArea(lessons, ['portal']).length, 1)
  assert.equal(lessonsForArea(lessons, ['database']).length, 0)
  assert.equal(lessonsForArea(lessons, []).length, 0)
})

test('context lessons: directive only appears when there are matching gaps', () => {
  assert.equal(buildContextLessonDirective([]), null)
  const d = buildContextLessonDirective([lesson()])
  assert.ok(d?.includes('KNOWN CONTEXT GAPS'))
  assert.ok(d?.includes('never re-derive scope'))
})

test('context lessons: file store round-trips so a gap survives into the next run', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-context-'))
  const path = join(dir, 'lessons.json')
  try {
    const store = new FileContextLessonStore(path)
    assert.equal(store.list().length, 0)
    const added = store.add({ area: ['engine'], missingFact: 'M', source: 'smith' })
    // A fresh store over the SAME path (a new process/run) sees the lesson.
    const nextRun = new FileContextLessonStore(path)
    assert.equal(nextRun.list().length, 1)
    assert.equal(nextRun.list()[0].id, added.id)
    nextRun.remove(added.id)
    assert.equal(existsSync(path), true, 'empty store still writes an empty array file')
    assert.equal(new FileContextLessonStore(path).list().length, 0)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

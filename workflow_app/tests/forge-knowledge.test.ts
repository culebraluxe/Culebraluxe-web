import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  authoritativeState,
  classifyKnowledge,
  selectKnowledgeForRole,
  type ForgeKnowledgeRecord,
} from '../forge/forge-knowledge'

// ENG-FORGE-HARDEN-01 — durable Forge knowledge classes (FACT/DECISION/STATE/
// LESSON), role-specific context selection, and the precedence rules (STATE
// over stale observations; story DECISION over LESSON; LESSON isolation).

let seq = 0
function rec(over: Partial<ForgeKnowledgeRecord>): ForgeKnowledgeRecord {
  seq += 1
  return {
    id: `k-${seq}`,
    class: 'FACT',
    scope: 'repo',
    storyId: null,
    title: 't',
    body: 'b',
    provenance: { kind: 'run', ref: 'run-1', atIso: '2026-01-01T00:00:00Z' },
    ...over,
  }
}

test('classification preserves FACT/DECISION/STATE/LESSON + repo/story scope', () => {
  const f = classifyKnowledge({
    id: 'f1', class: 'FACT', scope: 'repo', title: 'Neon branch is DEV', body: 'x',
    provenance: { kind: 'external', ref: 'db', atIso: '2026-01-01T00:00:00Z' },
  })
  assert.equal(f.class, 'FACT')
  assert.equal(f.scope, 'repo')
  assert.equal(f.storyId, null)

  const d = classifyKnowledge({
    id: 'd1', class: 'DECISION', scope: 'story', storyId: 'S1', title: 'adopt sql', body: 'x',
    provenance: { kind: 'architect', ref: 'a1', atIso: '2026-01-01T00:00:00Z' },
  })
  assert.equal(d.class, 'DECISION')
  assert.equal(d.storyId, 'S1')
})

test('role context: Smith receives NO LESSON; Architect may read LESSON as context', () => {
  const lesson = rec({ class: 'LESSON', scope: 'repo' })
  const fact = rec({ class: 'FACT' })

  const smith = selectKnowledgeForRole('smith', [lesson, fact])
  assert.ok(!smith.included.some((r) => r.class === 'LESSON'), 'smith has no LESSON authority')
  assert.equal(smith.excludedLessons.length, 1)
  assert.ok(smith.included.some((r) => r.class === 'FACT'))

  const architect = selectKnowledgeForRole('architect', [lesson, fact])
  assert.ok(architect.included.some((r) => r.class === 'LESSON'), 'architect may read lesson')
})

test('STATE precedence: a stale historical FACT is not surfaced over authoritative state', () => {
  const stale = rec({ class: 'FACT', scope: 'story', storyId: 'S1', stale: true, title: 'old observation' })
  const state = rec({ class: 'STATE', scope: 'story', storyId: 'S1', title: 'current lane = smith' })
  const sel = selectKnowledgeForRole('architect', [stale, state])
  assert.ok(!sel.included.some((r) => r.id === stale.id), 'stale observation suppressed')
  assert.equal(sel.state.length, 1)
  assert.equal(sel.state[0].title, 'current lane = smith')
})

test('DECISION precedence: a story DECISION suppresses a same-story LESSON for Architect', () => {
  const decision = rec({ class: 'DECISION', scope: 'story', storyId: 'S1', title: 'frozen contract' })
  const lesson = rec({ class: 'LESSON', scope: 'story', storyId: 'S1', title: 'old learning' })
  const sel = selectKnowledgeForRole('architect', [decision, lesson])
  assert.ok(sel.included.some((r) => r.class === 'DECISION'))
  assert.ok(!sel.included.some((r) => r.id === lesson.id), 'lesson must not override the decision')
  assert.ok(sel.excludedLessons.some((r) => r.id === lesson.id))
})

test('LESSON isolation: lessons never appear as authoritative STATE', () => {
  const lesson = rec({ class: 'LESSON', scope: 'story', storyId: 'S1' })
  const state = rec({ class: 'STATE', scope: 'story', storyId: 'S1' })
  const projected = authoritativeState('S1', [lesson, state])
  assert.deepEqual(projected.map((r) => r.class), ['STATE'])
})

test('backward compatibility: repo-global FACTs remain available; no second state source', () => {
  const repoFact = rec({ class: 'FACT', scope: 'repo', title: 'build requires pnpm' })
  const sel = selectKnowledgeForRole('smith', [repoFact])
  assert.ok(sel.included.some((r) => r.id === repoFact.id))
  assert.equal(sel.state.length, 0, 'selection never invents STATE')
})

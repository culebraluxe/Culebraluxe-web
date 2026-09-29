import { test } from 'node:test'
import assert from 'node:assert/strict'

import { sessionFieldsFromGitPacket } from './git-packet'
import { mergeStoryPackets, sessionFromStory } from './story-session'

const SAMPLE = `# ENG-FORGE-LANES — Split harness

## Goal
Route by job shape.

## Scope
Lane policy only.

## Architect brief
Neon is control plane. Git is what Grok reads.

## Context refs
- agent-runtime/lanes.ts

## Acceptance criteria
Grok can architect from git alone.
`

test('parses git packet headings into story fields', () => {
  const fields = sessionFieldsFromGitPacket(SAMPLE)
  assert.match(fields.goal ?? '', /job shape/)
  assert.match(fields.architectBrief ?? '', /Git is what Grok reads/)
  assert.match(fields.contextRefs ?? '', /lanes\.ts/)
  assert.match(fields.acceptanceCriteria ?? '', /git alone/)
})

test('git brief satisfies the Smith lane session', () => {
  const session = sessionFromStory(sessionFieldsFromGitPacket(SAMPLE))
  assert.equal(session.hasArchitectBrief, true)
  assert.equal(session.hasScoutPacket, true)
})

test('merge fills empty Neon brief from git', () => {
  const merged = mergeStoryPackets(
    { goal: 'from neon' },
    sessionFieldsFromGitPacket(SAMPLE),
  )
  assert.equal(merged.goal, 'from neon')
  assert.match(merged.architectBrief ?? '', /Grok/)
})

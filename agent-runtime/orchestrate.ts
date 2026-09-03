import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { buildLaneEnqueue } from './enqueue-lane'
import { sessionFieldsFromGitPacket } from './git-packet'
import type { LaneId } from './lanes'
import {
  mergeStoryPackets,
  type StoryPacketFields,
} from './story-session'

export type PickLaneInput = {
  story: StoryPacketFields
  lastFinishedRole?: string | null
}

/** A1 series picker. Never auto-queues Architect or Inspector. */
export function pickLane(input: PickLaneInput): LaneId {
  if (input.lastFinishedRole === 'builder') return 'assay'
  if (input.lastFinishedRole === 'scout') return 'smith'
  if (!present(input.story.architectBrief)) return 'scout'
  return 'smith'
}

export function loadGitPacket(
  storyId: string,
  repoRoot = process.cwd(),
): StoryPacketFields {
  const path = join(repoRoot, 'docs', 'agent', 'packets', `${storyId}.md`)
  try {
    return sessionFieldsFromGitPacket(readFileSync(path, 'utf8'))
  } catch {
    return {}
  }
}

export function storyFieldsFromBoardAndGit(
  story: StoryPacketFields,
  storyId: string,
  repoRoot = process.cwd(),
): StoryPacketFields {
  return mergeStoryPackets(story, loadGitPacket(storyId, repoRoot))
}

function present(value: string | null | undefined): boolean {
  return Boolean(value && value.trim().length > 0)
}

export function envelopeForLane(
  lane: LaneId,
  story: StoryPacketFields,
): ReturnType<typeof buildLaneEnqueue> {
  return buildLaneEnqueue({ lane, story })
}

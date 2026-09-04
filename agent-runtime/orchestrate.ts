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

/**
 * V6 series picker. A complete Architect contract always hands to Lead before
 * implementation. Lead's structured decision then chooses SOLO, Smith, split,
 * or Hold; this picker never skips that judgment gate.
 */
export function pickLane(input: PickLaneInput): LaneId {
  if (!present(input.story.architectBrief)) return 'scout'
  if (input.lastFinishedRole === 'builder') return 'lead'
  if (input.lastFinishedRole === 'architect') return 'lead'
  if (input.lastFinishedRole === 'scout') return 'lead'
  return 'lead'
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

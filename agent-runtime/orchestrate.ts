import { createHash } from 'node:crypto'
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

export function sha256Text(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex')
}

export type AdmitExecutableContractResult = {
  contract: StoryPacketFields
  missing: string[]
}

/**
 * Packet admission seam (V5-03R / Gemini #2).
 *
 * Merge durable Neon board truth (primary) over a parsed Git packet fallback,
 * stamping packetSha from the exact Git file bytes when Neon has not yet
 * persisted one. Neon durable packetSha always wins so a stale local checkout
 * can never erase the persisted executable contract.
 *
 * Missing executable facts are reported, never fabricated.
 */
export function admitExecutableContract(
  board: StoryPacketFields,
  packetMarkdown: string | null | undefined,
): AdmitExecutableContractResult {
  const packetFields = packetMarkdown ? sessionFieldsFromGitPacket(packetMarkdown) : {}
  const packetSha =
    board.packetSha?.trim() ||
    (packetMarkdown ? sha256Text(packetMarkdown) : null) ||
    packetFields.packetSha ||
    null
  const merged = mergeStoryPackets(board, packetFields)
  if (packetSha) merged.packetSha = packetSha
  const missing: string[] = []
  if (!merged.architectBrief?.trim()) missing.push('missing-architect-brief')
  if (!merged.acceptanceCriteria?.trim()) missing.push('missing-acceptance-criteria')
  if (!merged.testMode?.trim()) missing.push('missing-test-mode')
  if (!merged.assayCommands?.trim()) missing.push('missing-assay-plan')
  return { contract: merged, missing }
}

export function loadGitPacket(
  storyId: string,
  repoRoot = process.cwd(),
): StoryPacketFields {
  const path = join(repoRoot, 'docs', 'agent', 'packets', `${storyId}.md`)
  try {
    const markdown = readFileSync(path, 'utf8')
    const fields = sessionFieldsFromGitPacket(markdown)
    // Packet admission provenance: sha256 of the exact Git file bytes so Neon
    // can detect a stale local checkout without re-reading Git at runtime.
    // Neon durable packetSha always wins in mergeStoryPackets; this only fills
    // the gap when Neon has not yet persisted a contract.
    if (!fields.packetSha) {
      fields.packetSha = sha256Text(markdown)
    }
    return fields
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

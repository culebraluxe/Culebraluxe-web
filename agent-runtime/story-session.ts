import type { LaneSession } from './lane-policy'
import { skillInstructions } from './skills'
import { parsePacketLoop } from './loop'

export type StoryPacketFields = {
  architectBrief?: string | null
  contextRefs?: string | null
  acceptanceCriteria?: string | null
  goal?: string | null
  scope?: string | null
  testMode?: string | null
  assayCommands?: string | null
  packetSha?: string | null
  skills?: string | null
  loop?: string | null
}

function present(value: string | null | undefined): boolean {
  return Boolean(value && value.trim().length > 0)
}

export function mergeStoryPackets(
  primary: StoryPacketFields,
  fallback: StoryPacketFields,
): StoryPacketFields {
  const keys: (keyof StoryPacketFields)[] = [
    'architectBrief',
    'contextRefs',
    'acceptanceCriteria',
    'goal',
    'scope',
    'testMode',
    'assayCommands',
    'packetSha',
    'skills',
    'loop',
  ]
  const out: StoryPacketFields = { ...primary }
  for (const key of keys) {
    if (!present(out[key]) && present(fallback[key])) out[key] = fallback[key]
  }
  return out
}

export function sessionFromStory(
  story: StoryPacketFields,
  extras: Partial<LaneSession> = {},
): LaneSession {
  return {
    hasScoutPacket: present(story.contextRefs),
    hasArchitectBrief: present(story.architectBrief),
    hasAssayPlan: present(story.assayCommands),
    ...extras,
  }
}

export function storyPacketInstructions(story: StoryPacketFields): string {
  const parts: string[] = []
  if (present(story.goal)) parts.push(`Goal:\n${story.goal!.trim()}`)
  if (present(story.scope)) parts.push(`Scope:\n${story.scope!.trim()}`)
  if (present(story.architectBrief)) {
    parts.push(`Architect brief (authoritative):\n${story.architectBrief!.trim()}`)
  }
  if (present(story.acceptanceCriteria)) {
    parts.push(`Acceptance criteria:\n${story.acceptanceCriteria!.trim()}`)
  }
  if (present(story.contextRefs)) {
    parts.push(`Context refs (Scout packet):\n${story.contextRefs!.trim()}`)
  }
  const skills = skillInstructions(story.skills ?? null)
  if (skills) parts.push(skills)
  if (present(story.loop)) {
    const parsed = parsePacketLoop(story.loop)
    parts.push(
      `Loop (V3): intent=${parsed.intent}` +
        (parsed.parentRun ? ` parent_run=${parsed.parentRun}` : '') +
        (parsed.loopLabel ? ` ${parsed.loopLabel}` : '') +
        (parsed.failedCommands.length
          ? `\nFailed commands:\n${parsed.failedCommands.map((c) => `- ${c}`).join('\n')}`
          : '') +
        `\n${story.loop!.trim()}`,
    )
  }
  return parts.join('\n\n')
}

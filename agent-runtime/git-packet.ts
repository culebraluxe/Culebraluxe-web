import type { StoryPacketFields } from './story-session'

const HEADINGS: Record<string, keyof StoryPacketFields> = {
  goal: 'goal',
  scope: 'scope',
  'architect brief': 'architectBrief',
  'context refs': 'contextRefs',
  'acceptance criteria': 'acceptanceCriteria',
  'test mode': 'testMode',
  'assay commands': 'assayCommands',
  skills: 'skills',
}

export function sessionFieldsFromGitPacket(markdown: string): StoryPacketFields {
  const sections = splitSections(markdown)
  const fields: StoryPacketFields = {}
  for (const [heading, key] of Object.entries(HEADINGS)) {
    const value = sections.get(heading)
    if (value) fields[key] = value
  }
  return fields
}

export function splitSections(markdown: string): Map<string, string> {
  const out = new Map<string, string>()
  let current: string | null = null
  const buf: string[] = []

  const flush = () => {
    if (!current) return
    const text = buf.join('\n').trim()
    if (text) out.set(current, text)
    buf.length = 0
  }

  for (const raw of markdown.split(/\r?\n/)) {
    const heading = raw.match(/^##\s+(.+?)\s*$/)
    if (heading) {
      flush()
      current = heading[1].trim().toLowerCase()
      continue
    }
    if (current) buf.push(raw)
  }
  flush()
  return out
}

import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const KNOWN = ['neon', 'forms', 'workflow', 'ui'] as const
export type SkillId = (typeof KNOWN)[number]

export function parseSkillIds(raw: string | null | undefined): SkillId[] {
  if (!raw) return []
  const tokens = raw
    .toLowerCase()
    .split(/[\s,]+/)
    .map((t) => t.replace(/^[-*]+/, '').trim())
    .filter(Boolean)
  const out: SkillId[] = []
  for (const token of tokens) {
    if ((KNOWN as readonly string[]).includes(token) && !out.includes(token as SkillId)) {
      out.push(token as SkillId)
    }
  }
  return out
}

export function loadSkillText(
  ids: SkillId[],
  repoRoot = process.cwd(),
): string {
  const parts: string[] = []
  for (const id of ids) {
    const path = join(repoRoot, 'docs', 'agent', 'skills', `${id}.md`)
    try {
      parts.push(readFileSync(path, 'utf8').trim())
    } catch {
      parts.push(`Skill ${id}: file missing at docs/agent/skills/${id}.md`)
    }
  }
  return parts.join('\n\n')
}

export function skillInstructions(
  raw: string | null | undefined,
  repoRoot = process.cwd(),
): string | null {
  const ids = parseSkillIds(raw)
  if (ids.length === 0) return null
  const body = loadSkillText(ids, repoRoot)
  return `Skills (${ids.join(', ')}):\n${body}`
}

/**
 * ADD. New handoff marker. Maps onto live ArchitectFinding (seams ← scope).
 * Does not replace findingsFromArchitectEvidence — parent still uses that as fallback.
 */
import type { ArchitectFinding } from '../forge-shaping'

export const ARCHITECT_HANDOFF_PREFIX = 'FORGE_ARCHITECT_HANDOFF:'

export type ArchitectHandoffFinding = {
  id: string
  required: boolean
  summary: string
  preconditions: string[]
  scope: string[]
  /** NEW proof files this finding owes (repo-relative); must sit inside `scope`. */
  proofs: string[]
  postconditions: string[]
  classes: string[]
  risks: string[]
  hint?: ArchitectFinding['hint']
}

export type ArchitectHandoff = {
  version: 1
  baseRef: string
  findings: ArchitectHandoffFinding[]
}

function extractObject(text: string): string | null {
  const start = text.indexOf('{')
  if (start < 0) return null
  let depth = 0
  let inString = false
  let escaped = false
  for (let i = start; i < text.length; i++) {
    const ch = text[i]
    if (inString) {
      if (escaped) escaped = false
      else if (ch === '\\') escaped = true
      else if (ch === '"') inString = false
      continue
    }
    if (ch === '"') inString = true
    else if (ch === '{') depth++
    else if (ch === '}') {
      depth--
      if (depth === 0) return text.slice(start, i + 1)
    }
  }
  return null
}

function strings(v: unknown): string[] {
  if (!Array.isArray(v)) return []
  return v.filter((s): s is string => typeof s === 'string').map((s) => s.trim()).filter(Boolean)
}

export function parseArchitectHandoff(text: string | null | undefined): ArchitectHandoff | null {
  if (!text) return null
  const positions: number[] = []
  for (let at = text.indexOf(ARCHITECT_HANDOFF_PREFIX); at >= 0; at = text.indexOf(ARCHITECT_HANDOFF_PREFIX, at + 1)) {
    positions.push(at)
  }
  for (let i = positions.length - 1; i >= 0; i--) {
    const slice = extractObject(text.slice(positions[i] + ARCHITECT_HANDOFF_PREFIX.length))
    if (!slice) continue
    try {
      const raw = JSON.parse(slice) as Record<string, unknown>
      if (raw.version !== 1 || typeof raw.baseRef !== 'string' || !Array.isArray(raw.findings)) continue
      const findings: ArchitectHandoffFinding[] = []
      for (const row of raw.findings) {
        if (!row || typeof row !== 'object') return null
        const f = row as Record<string, unknown>
        const id = String(f.id ?? '').trim()
        const summary = String(f.summary ?? '').trim()
        if (!id || !summary) return null
        findings.push({
          id,
          required: f.required === true,
          summary,
          preconditions: strings(f.preconditions),
          scope: strings(f.scope ?? f.seams),
          proofs: strings(f.proofs),
          postconditions: strings(f.postconditions),
          classes: strings(f.classes),
          risks: strings(f.risks),
          hint: (String(f.hint ?? '') || undefined) as ArchitectFinding['hint'],
        })
      }
      return { version: 1, baseRef: raw.baseRef.trim(), findings }
    } catch {
      /* earlier marker */
    }
  }
  return null
}

/** Live lock type. Extra fields stay on the handoff in notes. */
export function handoffToFindings(handoff: ArchitectHandoff): ArchitectFinding[] {
  return handoff.findings.map((f) => ({
    id: f.id,
    summary: f.summary,
    required: f.required,
    seams: f.scope,
    ...(f.proofs.length > 0 ? { proofs: f.proofs } : {}),
    hint: f.hint,
  }))
}

export function lastMachineLine(raw: string, prefix: string): string | null {
  const lines = raw.split(/\r?\n/).map((s) => s.trim()).filter((s) => s.startsWith(prefix))
  return lines.length ? lines[lines.length - 1] : null
}

/** The handoff as the ONE machine line that carries the contract. */
export function renderArchitectHandoff(handoff: ArchitectHandoff): string {
  return `${ARCHITECT_HANDOFF_PREFIX} ${JSON.stringify(handoff)}`
}

export type ForgeLeadHint = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'

export type ForgeArchitectContract = {
  filesInScope: string[]
  filesOutOfScope: string[]
  acceptance: string[]
  /**
   * The acceptance-to-assertion mapping, written BEFORE the work: each acceptance clause names the
   * assertions in the frozen proof that assert it. A clause absent here has NO assertion behind it, so the
   * verdict for it is UNPROVEN — the clause is not allowed to pass on the cheapest reading.
   */
  acceptanceAssertions: Record<string, string[]>
  risk: string | null
  leadHint: ForgeLeadHint
}

const LEAD_HINTS: ReadonlySet<string> = new Set(['SOLO', 'SMITH', 'SPLIT', 'HOLD'])

function normalizeContractPath(path: string): string {
  return path.trim().replace(/^\.\//, '').replace(/\/+$/, '')
}

function cleanPaths(paths: unknown): string[] {
  if (!Array.isArray(paths)) return []
  return paths
    .filter((p): p is string => typeof p === 'string')
    .map(normalizeContractPath)
    .filter(Boolean)
}

/** Clause -> assertion refs. Anything that is not a string[] of refs is dropped, never coerced. */
function cleanAssertions(raw: unknown): Record<string, string[]> {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {}
  const out: Record<string, string[]> = {}
  for (const [clause, refs] of Object.entries(raw as Record<string, unknown>)) {
    if (!Array.isArray(refs)) continue
    const cleaned = refs
      .filter((r): r is string => typeof r === 'string')
      .map((r) => r.trim())
      .filter(Boolean)
    out[clause.trim()] = cleaned
  }
  return out
}

/**
 * A declaration as it arrives from a repository, a JSON column or a flag.
 *
 * ABSENT IS NOT EMPTY. `null`, a malformed value and an empty object all become `null` — the honest
 * absence of a declaration, which keeps the UNPROVEN `acceptance-map-missing` blocker unchanged. A
 * mapping that NAMES clauses but carries no assertion refs (`{ "clause": [] }`) is a real
 * declaration and is PRESERVED, because the ready gate must refuse it as unmapped rather than read
 * it as "nothing was declared".
 */
export function normalizeAcceptanceAssertions(raw: unknown): Record<string, string[]> | null {
  let value: unknown = raw
  if (typeof raw === 'string') {
    const text = raw.trim()
    if (!text) return null
    try {
      value = JSON.parse(text)
    } catch {
      return null
    }
  }
  const cleaned = cleanAssertions(value)
  return Object.keys(cleaned).length > 0 ? cleaned : null
}

/** Which declaration the resolver used, or null when neither declared a mapping. */
export type AcceptanceAssertionSource = 'handoff' | 'story' | null

export type ResolvedAcceptanceAssertions = {
  assertions: Record<string, string[]> | null
  source: AcceptanceAssertionSource
}

/**
 * THE ONE READER for the two declaration places.
 *
 * Precedence is STATED, not inferred: the handoff declaration (Architect/Lead, through the contract)
 * WINS; the story-author declaration is the fallback; neither yields `{ assertions: null, source: null }`
 * so a caller can tell "nothing was declared" from "a declaration was made". An empty or malformed
 * value on one side is treated as ABSENT, so it can never shadow a real mapping on the other — the
 * precedence order answers "which wins", never "which is present".
 */
export function resolveAcceptanceAssertions(input: {
  handoff?: unknown
  story?: unknown
}): ResolvedAcceptanceAssertions {
  const handoff = normalizeAcceptanceAssertions(input.handoff)
  if (handoff) return { assertions: handoff, source: 'handoff' }
  const story = normalizeAcceptanceAssertions(input.story)
  if (story) return { assertions: story, source: 'story' }
  return { assertions: null, source: null }
}

export function parseForgeArchitectContract(raw: unknown): ForgeArchitectContract | null {
  if (!raw || typeof raw !== 'object') return null
  const row = raw as Record<string, unknown>
  const leadHint = String(row.leadHint ?? 'SMITH').toUpperCase()
  if (!LEAD_HINTS.has(leadHint)) return null
  const filesInScope = cleanPaths(row.filesInScope)
  if (filesInScope.length === 0) return null
  return {
    filesInScope,
    filesOutOfScope: cleanPaths(row.filesOutOfScope),
    acceptance: Array.isArray(row.acceptance)
      ? row.acceptance.filter((item): item is string => typeof item === 'string' && item.trim().length > 0)
      : [],
    acceptanceAssertions: cleanAssertions(row.acceptanceAssertions),
    risk: typeof row.risk === 'string' && row.risk.trim() ? row.risk.trim() : null,
    leadHint: leadHint as ForgeLeadHint,
  }
}

export function architectContractFromNotes(notes: string | null | undefined): ForgeArchitectContract | null {
  if (!notes) return null
  const match = notes.match(/FORGE_ARCHITECT_CONTRACT:\s*(\{[\s\S]*\})/)
  if (!match) return null
  try {
    return parseForgeArchitectContract(JSON.parse(match[1]))
  } catch {
    return null
  }
}

export function pathViolatesArchitectContract(
  filePath: string,
  contract: ForgeArchitectContract,
): boolean {
  const normalized = normalizeContractPath(filePath)
  if (contract.filesOutOfScope.some((raw) => {
    const p = normalizeContractPath(raw)
    return normalized === p || normalized.startsWith(`${p}/`)
  })) {
    return true
  }
  if (contract.filesInScope.length === 0) return false
  return !contract.filesInScope.some((raw) => {
    const p = normalizeContractPath(raw)
    return normalized === p || normalized.startsWith(`${p}/`)
  })
}

export function architectContractViolations(
  touchedFiles: string[],
  contract: ForgeArchitectContract,
): string[] {
  return touchedFiles.filter((file) => pathViolatesArchitectContract(file, contract))
}

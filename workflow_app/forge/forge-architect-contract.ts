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

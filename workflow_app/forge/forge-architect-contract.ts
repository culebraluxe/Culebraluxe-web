export type ForgeLeadHint = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'

export type ForgeArchitectContract = {
  filesInScope: string[]
  filesOutOfScope: string[]
  acceptance: string[]
  risk: string | null
  leadHint: ForgeLeadHint
}

const LEAD_HINTS: ReadonlySet<string> = new Set(['SOLO', 'SMITH', 'SPLIT', 'HOLD'])

function cleanPaths(paths: unknown): string[] {
  if (!Array.isArray(paths)) return []
  return paths
    .filter((p): p is string => typeof p === 'string')
    .map((p) => p.trim().replace(/^\.\//, ''))
    .filter(Boolean)
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
  const normalized = filePath.trim().replace(/^\.\//, '')
  if (contract.filesOutOfScope.some((p) => normalized === p || normalized.startsWith(`${p}/`))) {
    return true
  }
  if (contract.filesInScope.length === 0) return false
  return !contract.filesInScope.some((p) => normalized === p || normalized.startsWith(`${p}/`))
}

export function architectContractViolations(
  touchedFiles: string[],
  contract: ForgeArchitectContract,
): string[] {
  return touchedFiles.filter((file) => pathViolatesArchitectContract(file, contract))
}

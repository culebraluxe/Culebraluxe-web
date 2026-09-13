import { fileOf } from '../shared/path'
import type { SmithAssignment, SmithChunk } from './types'

/** Adapt a greenfield Lead assignment into the Smith lock. */
export function assignmentFromLead(input: {
  id: string
  findingIds: string[]
  chunks: Array<{
    id: number
    preconditions?: string[]
    scope: string[]
    postconditions?: string[]
    classes?: string[]
    risks?: string[]
    proof: string
  }>
  prohibitedScope?: string[]
}): SmithAssignment {
  const chunks: SmithChunk[] = input.chunks.map((c) => ({
    id: c.id,
    preconditions: c.preconditions ?? [],
    scope: c.scope,
    postconditions: c.postconditions ?? [],
    classes: c.classes ?? [],
    risks: c.risks ?? [],
    proof: c.proof,
  }))
  const allowed = [...new Set(chunks.flatMap((c) => c.scope.map(fileOf).filter((s): s is string => !!s)))]
  return {
    id: input.id,
    findingIds: input.findingIds,
    chunks,
    allowedScope: allowed,
    prohibitedScope: input.prohibitedScope ?? [],
  }
}

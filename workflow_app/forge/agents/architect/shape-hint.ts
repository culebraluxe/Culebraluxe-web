import { fileOf, overlap } from '../shared/path'

/**
 * Advisory only. Never veto a valid LEAD_ROUTING line. Printed into the Lead prompt.
 *
 * Takes the minimum shape it actually reads (id/required/hint/scope) so it can be
 * called with either the Architect handoff or the live routing context's findings.
 */
export function seamGroupHint(
  findings: Array<{ id: string; required: boolean; hint?: string; scope: string[] }>,
): {
  groups: string[][]
  text: string
} {
  const required = findings.filter((f) => f.required && f.hint !== 'HOLD')
  const parent = required.map((_, i) => i)
  const find = (i: number): number => {
    let r = i
    while (parent[r] !== r) r = parent[r]
    return r
  }
  const unite = (a: number, b: number) => {
    const ra = find(a)
    const rb = find(b)
    if (ra !== rb) parent[rb] = ra
  }
  for (let i = 0; i < required.length; i++) {
    for (let j = i + 1; j < required.length; j++) {
      const a = required[i].scope.map((s) => fileOf(s)).filter((s): s is string => !!s)
      const b = required[j].scope.map((s) => fileOf(s)).filter((s): s is string => !!s)
      if (a.some((x) => b.some((y) => overlap(x, y)))) unite(i, j)
    }
  }
  const buckets = new Map<number, string[]>()
  required.forEach((f, i) => {
    const root = find(i)
    const list = buckets.get(root) ?? []
    list.push(f.id)
    buckets.set(root, list)
  })
  const groups = [...buckets.values()]
  const text =
    groups.length <= 1
      ? `Architect seams form one group (${required.map((f) => f.id).join(', ') || 'none'}). That is a hint, not a route.`
      : `Architect seams form ${groups.length} groups: ${groups.map((g) => g.join('+')).join(' | ')}. Lead may still SMITH as serial chunks if they share a worker; SPLIT only when write surfaces do not overlap.`
  return { groups, text }
}

// Pure, DB-free core of the workflow start boundary — separated so it can be
// tested without importing the CulebraLuxe or engine database clients.

export type StartWorkflowDeps = {
  findActive: (dealId: string) => Promise<string | null>
  readFacts: (dealId: string) => Promise<Record<string, any> | null>
  start: (dealId: string, facts: Record<string, any>) => Promise<string>
}

export async function startWorkflowCore(
  dealId: string,
  deps: StartWorkflowDeps,
): Promise<{ instanceId: string; started: boolean }> {
  const existing = await deps.findActive(dealId)
  if (existing) return { instanceId: existing, started: false }

  const facts = (await deps.readFacts(dealId)) ?? {}

  try {
    const instanceId = await deps.start(dealId, facts)
    return { instanceId, started: true }
  } catch (err) {
    if (isUniqueViolation(err)) {
      const winner = await deps.findActive(dealId)
      if (winner) return { instanceId: winner, started: false }
    }
    throw err
  }
}

export function isUniqueViolation(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as { code?: string }).code === '23505'
  )
}

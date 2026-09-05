export function parseForgeSpendCapUsd(
  raw: string | null | undefined = process.env.FORGE_STORY_SPEND_CAP_USD,
): number | null {
  if (raw === undefined || raw === null || raw.trim() === '') return null
  const value = Number(raw)
  if (!Number.isFinite(value) || value < 0) return null
  return value
}

export function forgeSpendShouldHold(input: {
  spendUsd: number | null
  capUsd: number | null
}): boolean {
  if (input.spendUsd === null || input.capUsd === null) return false
  return input.spendUsd > input.capUsd
}

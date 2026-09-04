// ---------------------------------------------------------------------------
// Forge model price table (spend vision).
//
// Relative per-run cost weights for Lead PRE grade decisions. Values are
// coarse operator-set weights, NOT live vendor quotes: flash-class models
// cost ~1 unit, pro-class ~10x. The table is data so prices can be retuned
// without touching prompt text; Lead sees only the rendered lines.
// ---------------------------------------------------------------------------

export type ModelPriceEntry = {
  /** Exact team-mapped model id (`provider/model`) or `forge` for free lanes. */
  model: string
  /** Relative cost weight in flash-units. Zero = free (deterministic Assay). */
  weight: number
  /** Short human label for Lead-facing cost lines. */
  label: string
}

export const MODEL_PRICE_TABLE: ModelPriceEntry[] = [
  { model: 'deepseek/deepseek-v4-flash', weight: 1, label: 'flash (cheap volume)' },
  { model: 'deepseek/deepseek-chat', weight: 10, label: 'pro (10x — judgment only)' },
  { model: 'forge/deterministic-assay', weight: 0, label: 'deterministic (free)' },
]

export function modelPriceFor(model: string | null | undefined): ModelPriceEntry | null {
  const key = (model ?? '').trim().toLowerCase()
  if (!key) return null
  return MODEL_PRICE_TABLE.find((entry) => entry.model === key) ?? null
}

/** Render the 5-line relative-cost table Lead PRE sees. Never prices. */
export function renderModelCostLines(): string[] {
  return [
    'Relative model cost (operator-set weights, not vendor quotes):',
    ...MODEL_PRICE_TABLE.map(
      (entry) => `- ${entry.model}: ${entry.weight}x (${entry.label})`,
    ),
    'Prefer the cheapest grade that can soundly do the work. Pro is for judgment vetoes and hard builds, never volume.',
  ]
}

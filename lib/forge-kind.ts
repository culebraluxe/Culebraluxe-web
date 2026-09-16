// ---------------------------------------------------------------------------
// FORGE KIND + MODEL POLICY — the two doors every piece of work goes through before a
// lane speaks (ENG-FORGE-FACTORY-01 Phase 1).
//
// SIX KINDS, TWO POLICIES, and no more. The packet's stop condition is explicit: "kind
// routing starts choosing providers per token" is a HOLD. So the whole decision surface
// is this file — a closed set, a policy per kind, a first lane, and ONE table mapping the
// two policies onto model names that already exist in `agent-runtime/model-prices.ts`.
// There is deliberately no per-story model field and no third policy, because the moment
// there is, somebody has to reason about model choice per story, which is the thing the
// packet is trying to stop.
//
// WHY THE DEFAULTS MATTER: `cheap` is the default policy so the unattended night run is
// cheap without anyone remembering to say so, and `fix` is the default kind because "we
// know it is broken" is the honest description of most work that arrives on its own.
// ---------------------------------------------------------------------------

export const FORGE_KINDS = ['qa', 'fix', 'feature', 'crm', 'judgment', 'learn'] as const
export type ForgeKind = (typeof FORGE_KINDS)[number]

export const FORGE_MODEL_POLICIES = ['cheap', 'judgment'] as const
export type ForgeModelPolicy = (typeof FORGE_MODEL_POLICIES)[number]

export const DEFAULT_FORGE_KIND: ForgeKind = 'fix'
export const DEFAULT_MODEL_POLICY: ForgeModelPolicy = 'cheap'

/**
 * THE POLICY AXIS AND THE LAB AXIS MUST NOT SHARE A WORD (Grok, 2026-09-15).
 *
 * `judgment` as a POLICY means "dear DeepSeek chat" — it is a SPEND grade. `judgment-lab` means Grok, a
 * different seat entirely. Printing `judgment` in a log line, a cockpit chip or an ROI row reads as the lab,
 * so every human-facing label says `dear` (or `cheap`) while the stored value stays exactly as it is. This is
 * a display name: never a second policy, never a reason to route a lane to another model.
 */
export function forPolicyLabel(policy: string): string {
  // Unknown values pass through UNCHANGED: `unrecorded` is a real, honest state (routing that was never
  // recorded) and must never be relabelled as a policy somebody chose. Only the one collision is renamed.
  return policy === 'judgment' ? 'dear' : policy
}

/** The first lane a kind starts in. It does NOT replace Lead's SMITH/SPLIT decision later. */
export type ForgeLaneStart = 'Scout' | 'Architect' | 'Assay'

export type KindRouting = {
  /** Default policy when the row does not say. */
  policy: ForgeModelPolicy
  laneStart: ForgeLaneStart
  /** One line, in the words a roster can show. */
  label: string
  example: string
}

/**
 * The packet's Object 1 table, in code. Six kinds, nothing else.
 */
export const KIND_ROUTING: Record<ForgeKind, KindRouting> = {
  qa: { policy: 'cheap', laneStart: 'Scout', label: 'question', example: '"what does X do"' },
  fix: { policy: 'cheap', laneStart: 'Scout', label: 'fix', example: 'silent failure, broken write' },
  feature: { policy: 'judgment', laneStart: 'Architect', label: 'feature', example: 'new surface' },
  crm: { policy: 'cheap', laneStart: 'Scout', label: 'crm follow-up', example: 'HubSpot / WhatsApp follow-up' },
  judgment: { policy: 'judgment', laneStart: 'Architect', label: 'judgment call', example: 'constitution, scoring, HOLD' },
  learn: { policy: 'cheap', laneStart: 'Assay', label: 'learn', example: 'hunter → packet or batch row' },
}

/**
 * POLICY -> MODEL, one table, reusing names that already exist in `agent-runtime/model-prices.ts`
 * (where `deepseek-v4-flash` is weighted 1 and `deepseek-chat` is weighted 10, "judgment only").
 * Two rows. Adding a third means a human is choosing providers per story, which is a HOLD.
 */
export const MODEL_FOR_POLICY: Record<ForgeModelPolicy, { model: string; note: string }> = {
  cheap: { model: 'deepseek/deepseek-v4-flash', note: 'volume work: cheap, fast, good enough' },
  judgment: { model: 'deepseek/deepseek-chat', note: 'architecture, scoring, HOLD decisions' },
}

export function isForgeKind(value: unknown): value is ForgeKind {
  return typeof value === 'string' && (FORGE_KINDS as readonly string[]).includes(value)
}

export function isModelPolicy(value: unknown): value is ForgeModelPolicy {
  return typeof value === 'string' && (FORGE_MODEL_POLICIES as readonly string[]).includes(value)
}

/** Unknown or missing kind reads as the default rather than throwing: the column already has one. */
export function asForgeKind(value: unknown): ForgeKind {
  return isForgeKind(value) ? value : DEFAULT_FORGE_KIND
}

/** A policy that is not one of the two — including null on a legacy row — reads as `cheap`. */
export function asModelPolicy(value: unknown): ForgeModelPolicy {
  return isModelPolicy(value) ? value : DEFAULT_MODEL_POLICY
}

/** The routing decision for a kind: policy + first lane + the words to show. */
export function routingFor(kind: unknown): KindRouting {
  return KIND_ROUTING[asForgeKind(kind)]
}

/**
 * The policy a batch should run under.
 *
 * The rule from the packet: "Night / `fireDueForgeBatches` defaults `model_policy=cheap` unless the
 * row says `judgment`". So the row wins when it is a real policy, and anything else is cheap — which
 * is exactly the column default, restated here so a caller cannot pass a typo through to a model.
 */
export function policyForBatch(rowPolicy: unknown): ForgeModelPolicy {
  return asModelPolicy(rowPolicy)
}

/** The model name for a policy, for the worker log line and the run record. */
export function modelForPolicy(policy: unknown): string {
  return MODEL_FOR_POLICY[asModelPolicy(policy)].model
}

/** One line for a worker log or a roster: `fix/cheap → deepseek/deepseek-v4-flash · Scout`. */
export function describeRouting(kind: unknown, policy?: unknown): string {
  const resolvedKind = asForgeKind(kind)
  const routing = KIND_ROUTING[resolvedKind]
  const resolvedPolicy = policy === undefined ? routing.policy : asModelPolicy(policy)
  return `${resolvedKind}/${forPolicyLabel(resolvedPolicy)} → ${MODEL_FOR_POLICY[resolvedPolicy].model} · starts ${routing.laneStart}`
}

/**
 * The kind mix of a staged batch, in the order the kinds are declared.
 *
 * The cockpit's batch roster is per batch and the kind is per member, so the honest summary is a
 * count of kinds (not one kind). Deterministic ordering matters more than it looks: this string is
 * rendered on a screen that refreshes every 30 seconds, and a summary whose order wanders reads as
 * a change when nothing changed. Empty input returns an empty string, so the caller can omit it.
 */
export function summarizeKinds(kinds: readonly unknown[]): string {
  const counts = new Map<ForgeKind, number>()
  for (const kind of kinds) {
    const resolved = asForgeKind(kind)
    counts.set(resolved, (counts.get(resolved) ?? 0) + 1)
  }
  return FORGE_KINDS.filter((kind) => counts.has(kind))
    .map((kind) => (counts.get(kind) === 1 ? kind : `${kind} ×${counts.get(kind)}`))
    .join(', ')
}

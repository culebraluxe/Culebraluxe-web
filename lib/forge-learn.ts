// ---------------------------------------------------------------------------
// FORGE LEARN LOOP — traces write work (ENG-FORGE-FACTORY-01 Phase 3, Object 3).
//
// Assay already exists; this gives it a job that is not "run these tests". On each unattended pass the
// worker runs the silent-failure hunter over the files that changed inside a bounded window, adds the
// stale-claim query, and may file AT MOST ONE learn item. Lead and Architect still decide SMITH vs HOLD.
// The loop files work; it never ships code, never merges, and never promotes a decision.
//
// THREE RULES, THREE KINDS OF ENFORCEMENT, on purpose:
//
//   1. AT MOST ONE PER PASS is a function that returns one item (`decideLearnItem`) - a cap in code,
//      because "per pass" is a property of the pass, not of the data.
//   2. NEVER TWICE FOR THE SAME PATTERN is a partial unique index in migration 181 - a cap in the
//      database, because a check-then-insert in code is a race between two worker passes and a lie the
//      moment they overlap.
//   3. ASSAY DOES NOT SHIP CODE is the shape of the thing filed: a story, a packet, and instructions
//      that say who decides. There is no commit path in this module to disable.
//
// WHY KEYS ARE SOMETIMES PER-FILE: a per-pattern key for a file finding ("empty-catch exists somewhere")
// means one filed item silences every other file for the window, and the second real defect hides behind
// the first ticket. So file findings key on `pattern:path`. A stale claim is one operational condition,
// not a location, so it keys on `stale-claim`.
// ---------------------------------------------------------------------------

export const LEARN_PATTERNS = [
  'stale-claim',
  'empty-catch',
  'swallowed-catch',
  'console-error-without-capture',
  'bare-500-in-catch',
] as const
export type LearnPattern = (typeof LEARN_PATTERNS)[number]

export type LearnSeverity = 'P0' | 'normal'

/**
 * The packet names three P0 shapes: "silent write / abandoned claim / digest without row". An abandoned
 * claim is the one the factory can already detect deterministically, so it is P0 here, and it is the only
 * P0 that bypasses staging - the code-level findings wait in the batch like any other queued work.
 */
export const LEARN_PATTERN_SEVERITY: Record<LearnPattern, LearnSeverity> = {
  'stale-claim': 'P0',
  'empty-catch': 'normal',
  'swallowed-catch': 'normal',
  'console-error-without-capture': 'normal',
  'bare-500-in-catch': 'normal',
}

/** The packet's cap: at most one auto-filed learn item per worker pass. */
export const MAX_LEARN_ITEMS_PER_PASS = 1

/** The packet's window cap: "since last successful learn run, max 24h". */
export const DEFAULT_LEARN_WINDOW_HOURS = 24

export type LearnCandidate = {
  pattern: LearnPattern
  /** The pattern key: `stale-claim`, or `empty-catch:app/api/x/route.ts`. */
  key: string
  severity: LearnSeverity
  /** One line, for the story title. */
  title: string
  /** Evidence: `path:line` for a code finding, work-item ids for a stale claim. */
  evidence: string[]
  /** How many hits this pattern produced in the window, for ordering and for the packet. */
  hitCount: number
  firstSeen: string
  lastSeen: string
}

export function isLearnPattern(value: unknown): value is LearnPattern {
  return typeof value === 'string' && (LEARN_PATTERNS as readonly string[]).includes(value)
}

/** The dedupe key. `detail` is a path for code findings and null for whole-system patterns. */
export function learnPatternKey(pattern: LearnPattern, detail?: string | null): string {
  const trimmed = (detail ?? '').trim().replace(/^\/+/, '')
  return trimmed ? `${pattern}:${trimmed}` : pattern
}

/** A stable, readable story id: `LEARN-EMPTY-CATCH-APP-API-X-ROUTE-TS-20260915`. */
export function learnStoryId(key: string, dateIso: string): string {
  const day = dateIso.slice(0, 10).replace(/-/g, '')
  const slug = key.toUpperCase().replace(/[^A-Z0-9]+/g, '-').replace(/^-+|-+$/g, '')
  return `LEARN-${slug.slice(0, 60)}-${day}`
}

/**
 * WHICH ITEM GETS FILED, IF ANY.
 *
 * P0 first, then the pattern with the most hits, then alphabetically by key so two workers looking at the
 * same evidence choose the same item. Anything skipped for the cap is returned as `deferred` rather than
 * dropped: a burst of ten findings that files one item and says nothing is how the other nine disappear.
 */
export function decideLearnItem(input: {
  candidates: readonly LearnCandidate[]
  /** Pattern keys that already have an open item (work item or staged member). */
  openPatternKeys: ReadonlySet<string>
  max?: number
}): { filed: LearnCandidate | null; deferred: LearnCandidate[]; skipped: LearnCandidate[] } {
  const max = input.max ?? MAX_LEARN_ITEMS_PER_PASS
  const skipped = input.candidates.filter((candidate) => input.openPatternKeys.has(candidate.key))
  const open = input.candidates
    .filter((candidate) => !input.openPatternKeys.has(candidate.key))
    .sort((a, b) => {
      if (a.severity !== b.severity) return a.severity === 'P0' ? -1 : 1
      if (a.hitCount !== b.hitCount) return b.hitCount - a.hitCount
      return a.key.localeCompare(b.key)
    })
  return {
    filed: open[0] ?? null,
    deferred: open.slice(max),
    skipped,
  }
}

/** The window anchor: `since the last successful learn run, max 24h`. */
export type LearnAnchor = { at: string; lastKey: string | null }

export function parseLearnAnchor(raw: string | null | undefined): LearnAnchor | null {
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as Partial<LearnAnchor>
    if (!parsed?.at || Number.isNaN(Date.parse(parsed.at))) return null
    return { at: parsed.at, lastKey: parsed.lastKey ?? null }
  } catch {
    return null
  }
}

/** The start of the window: the anchor, or `windowHours` ago when there is none, capped either way. */
export function learnWindowStart(
  anchor: LearnAnchor | null,
  now: Date,
  windowHours: number = DEFAULT_LEARN_WINDOW_HOURS,
): Date {
  const floor = new Date(now.getTime() - Math.max(1, windowHours) * 3_600_000)
  if (!anchor) return floor
  const anchored = new Date(anchor.at)
  return anchored.getTime() < floor.getTime() ? floor : anchored
}

/**
 * The learn packet, from the template the packet calls for: pattern, first/last seen, evidence ids,
 * proposed decision or fix surface.
 *
 * Shaped like every other packet in `docs/agent/packets/` because `pnpm forge:packet-lint` scans it: the
 * skills section names a real pack, the loop line is `intent: grow`, and the language never tells Scout,
 * Assay or Inspector to commit. What it does NOT contain is an acceptance criterion: the fix surface is
 * the Architect's call, and a generated acceptance would be this loop deciding scope.
 */
export function renderLearnPacket(
  candidate: LearnCandidate,
  context: { storyId: string; windowStart: string; windowEnd: string },
): string {
  const lines: string[] = []
  lines.push(`# ${context.storyId} — learn: ${candidate.key}`)
  lines.push('')
  lines.push('## Goal')
  lines.push('')
  lines.push(`Decide what "${candidate.key}" means for this repository, then either fix it or record why it stands.`)
  lines.push('')
  lines.push('## Why')
  lines.push('')
  lines.push('Filed automatically by the learn loop (ENG-FORGE-FACTORY-01 Phase 3) from the unattended worker pass.')
  lines.push(
    `Pattern \`${candidate.pattern}\`, severity ${candidate.severity}, ${candidate.hitCount} hit(s) in the window ` +
      `${context.windowStart} → ${context.windowEnd}.`,
  )
  lines.push(`First seen ${candidate.firstSeen}, last seen ${candidate.lastSeen}.`)
  lines.push('')
  lines.push('## Scope')
  lines.push('')
  lines.push('Whatever the pattern actually is, once verified in source. The evidence below is where the loop saw it,')
  lines.push('not a statement that it is still true: verify before fixing, and say so if it is a false positive.')
  lines.push('')
  lines.push('## Evidence (from the trace, not authority)')
  lines.push('')
  for (const item of candidate.evidence) lines.push(`- \`${item}\``)
  lines.push('')
  lines.push('## Architect brief')
  lines.push('')
  lines.push('The learn loop files work; it does not decide scope, and it does not ship code. Lead and Architect')
  lines.push('choose SMITH or HOLD from the verified pattern, and the fix surface is the Architect to name.')
  lines.push('')
  lines.push('## Acceptance criteria')
  lines.push('')
  lines.push('To be written after the pattern is verified. A generated acceptance criterion would be this loop')
  lines.push('making a scope decision it is not allowed to make.')
  lines.push('')
  lines.push('## Preconditions')
  lines.push('')
  lines.push('The pattern is reproducible, or the run explains why it is not.')
  lines.push('')
  lines.push('## Postconditions')
  lines.push('')
  lines.push('Either the cause is fixed with a test that fails before and passes after, or a decision records why')
  lines.push('the pattern stands. A silent close is not an outcome.')
  lines.push('')
  lines.push('## Skills')
  lines.push('')
  lines.push('workflow')
  lines.push('')
  lines.push('## Loop')
  lines.push('')
  lines.push('intent: grow')
  lines.push('loop: 1/3')
  lines.push('')
  lines.push('## Test mode')
  lines.push('')
  lines.push('SCOPED')
  lines.push('')
  lines.push('## Assay commands')
  lines.push('')
  lines.push('Named by the Architect with the fix surface. Until then this packet names none, so Assay does not launch.')
  lines.push('')
  return lines.join('\n')
}

/**
 * The instructions that travel with the filed item.
 *
 * They exist because the item is created by a machine and picked up by a lane that did not choose it: the
 * text says who decides and what the item is not for.
 */
export function buildLearnItemInstructions(candidate: LearnCandidate): string {
  return [
    `Filed by the learn loop: pattern ${candidate.key} (${candidate.hitCount} hit(s)).`,
    'Lead and Architect decide SMITH or HOLD; the loop does not choose the fix.',
    'Assay does not ship code. Never auto-merge, never auto-promote a decision.',
    `Evidence: ${candidate.evidence.slice(0, 5).join(', ') || '(none recorded)'}.`,
  ].join(' ')
}

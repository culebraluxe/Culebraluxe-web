// ---------------------------------------------------------------------------
// ENG-FORGE-SYNC-01 — ship-time board sync.
//
// The problem this exists to end: PROJECTS-WORKSPACE-01..12 shipped as merged
// commits while the PROD board still read Planned at 20-75%, because no run or
// work-item rows existed for that series and nothing connected shipped code to
// the board. Twelve stories had to be reconciled by hand.
//
// Four rules, straight from the story's acceptance criteria:
//   1. RUN PROVENANCE   — every run's execution_environment is surfaced, so a
//                         board entry can never imply evidence it cannot point at.
//   2. SHIP-TIME SYNC   — shipped work derives Complete from durable evidence.
//   3. ENVIRONMENT      — Forge executes against PROD. A non-PROD target FAILS
//                         CLOSED, and a run that happened elsewhere is visible.
//   4. NO INVENTED DATA — absent evidence is stated as absent, never estimated.
//
// This module is PURE (no database, no git, no filesystem): it derives the
// decision and composes the audit note. The adapter that applies it lives in
// scripts/forge-board-sync.ts.
// ---------------------------------------------------------------------------

import type { ExecutionEnvironment } from '../../lib/execution-target'

/** Forge executes against PROD. This is a rule, not a default. */
export const FORGE_EXECUTION_ENVIRONMENT: ExecutionEnvironment = 'PROD'

/**
 * Statuses a human parks a story in on purpose. Shipped code does NOT repeal
 * that decision, and a board sync must never silently overwrite it — it reports
 * the state (`held-shipped`) so a person can release the hold deliberately.
 */
export const SYNC_PROTECTED_STATUSES: ReadonlySet<string> = new Set(['Hold', 'Deferred'])

export class ForgeEnvironmentError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeEnvironmentError'
  }
}

/**
 * Fail closed: a Forge run may only execute against PROD. `TEST` is permitted
 * only when the caller explicitly declares it a test context, so unit tests can
 * exercise the guard without pretending to be production.
 *
 * Aliases match the canonical seam in lib/execution-target.ts (PRODUCTION and
 * DEVELOPMENT are accepted there, so they are accepted here).
 */
export function assertForgeExecutionTarget(
  target: string | null | undefined,
  options?: { allowEnvironment?: ExecutionEnvironment },
): ExecutionEnvironment {
  const normalized = normalizeExecutionTarget(target)
  if (normalized === FORGE_EXECUTION_ENVIRONMENT) return 'PROD'
  if (options?.allowEnvironment && normalized === options.allowEnvironment) {
    return options.allowEnvironment
  }
  throw new ForgeEnvironmentError(
    `Forge runs against PROD only: resolved execution target was ${JSON.stringify(target ?? null)}. ` +
      'Refusing to launch (fail closed). See docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md section 0.',
  )
}

/** Canonicalize a raw execution-environment token; unknown values stay as-is. */
export function normalizeExecutionTarget(
  target: string | null | undefined,
): string {
  const raw = (target ?? '').trim().toUpperCase()
  if (raw === 'PRODUCTION') return 'PROD'
  if (raw === 'DEVELOPMENT') return 'DEV'
  return raw
}

// --- 1. Run provenance ------------------------------------------------------

export type RunEnvironmentSummary = {
  byEnvironment: Record<string, number>
  /** Environments other than PROD that a run actually executed in. */
  nonProdEnvironments: string[]
  /** Runs whose environment was never recorded at all. */
  unknownCount: number
}

/**
 * Surface where runs actually ran. An unrecorded environment is reported as
 * UNKNOWN rather than assumed to be production — the whole failure mode this
 * story closes was a run in the wrong place being indistinguishable from one in
 * the right place.
 */
export function summarizeRunEnvironments(
  environments: Array<string | null | undefined>,
): RunEnvironmentSummary {
  const byEnvironment: Record<string, number> = {}
  let unknownCount = 0
  for (const raw of environments) {
    const value = (raw ?? '').trim().toUpperCase()
    if (!value) {
      unknownCount += 1
      byEnvironment.UNKNOWN = (byEnvironment.UNKNOWN ?? 0) + 1
      continue
    }
    byEnvironment[value] = (byEnvironment[value] ?? 0) + 1
  }
  const nonProdEnvironments = Object.keys(byEnvironment)
    .filter((env) => env !== FORGE_EXECUTION_ENVIRONMENT && env !== 'UNKNOWN')
    .sort()
  return { byEnvironment, nonProdEnvironments, unknownCount }
}

// --- Ship evidence ----------------------------------------------------------

export type ShipEvidence = {
  storyId: string
  /**
   * SHIPPING commits — matched the story id and are not docs-only.
   * A packet/notes commit proves a story was WRITTEN, not that it was built.
   */
  commits: string[]
  /** Commits that merely mention the story (packets, memory notes). Never completion evidence. */
  docsOnly?: string[]
}

export type RunEvidence = {
  runCount: number
  itemCount: number
  environments: Array<string | null | undefined>
}

/**
 * Match a story id as a WHOLE TOKEN. A bare substring match is wrong: the id
 * `OPS-11` appears inside `OPS-11A`, so `feat(ops): OPS-11A — ...` would have
 * completed a different story. Boundaries are any non-identifier character.
 */
export function storyIdMatcher(storyId: string): RegExp | null {
  const id = storyId.trim()
  if (!id) return null
  const escaped = id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return new RegExp(`(^|[^A-Za-z0-9-])${escaped}(?![A-Za-z0-9-])`, 'i')
}

/**
 * Extract the commits that shipped a story from `git log --oneline` output.
 * Story ids are already the link between merged code and the board (the
 * convention in use: `PROJECTS-WORKSPACE-12`, `integrate(projects-workspace-10)`),
 * so matching is case-insensitive and anchored on the whole id, not a prefix.
 */
export function parseShipCommits(storyId: string, gitLogOutput: string): string[] {
  const matcher = storyIdMatcher(storyId)
  if (!matcher) return []
  return gitLogOutput
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && matcher.test(line))
}

/**
 * Is this commit SHIPPING evidence, or merely a document about the work?
 *
 * Writing a story is not building it, and a packet is a document:
 *   `docs(forge): story packet for PROJECTS-WORKSPACE-13`   (typed docs)
 *   `ENG-FORGE-V5-11: lead dev qa topology packet`          (no type at all)
 *   `ENG-PROJECTS-ANCHOR-02: ... packet + seed ...`         (typed chore)
 * All three were observed on live data and all three were false completions.
 */
export function isShippingCommit(line: string): boolean {
  const subject = line.replace(/^\S+\s+/, '')
  const typeMatch = subject.match(/^([A-Za-z]+)\s*[(:]/)
  const type = typeMatch ? typeMatch[1].toLowerCase() : ''
  if (type === 'docs') return false
  if (/\bpacket\b/i.test(subject)) return false
  return true
}

/**
 * Split matched commits into SHIPPING evidence and docs-only noise.
 * Only `shipping` may complete a story.
 */
export function classifyShipCommits(
  storyId: string,
  gitLogOutput: string,
): { shipping: string[]; docsOnly: string[] } {
  const shipping: string[] = []
  const docsOnly: string[] = []
  for (const line of parseShipCommits(storyId, gitLogOutput)) {
    if (isShippingCommit(line)) shipping.push(line)
    else docsOnly.push(line)
  }
  return { shipping, docsOnly }
}

/** Commits that count as shipping evidence. */
export function shippingCommits(storyId: string, gitLogOutput: string): string[] {
  return classifyShipCommits(storyId, gitLogOutput).shipping
}

// --- 2/4. The derivation ----------------------------------------------------

export type BoardSyncStory = {
  id: string
  status: string
  completion: number | null
}

export type BoardSyncAction = 'complete' | 'no-change'

export type BoardSyncDecision = {
  storyId: string
  action: BoardSyncAction
  /** Set only when the action writes a completion. */
  completion: number | null
  /** Short machine reason, safe to log/count. */
  reason: string
  /** Durable audit note. Null when nothing is written. */
  note: string | null
  provenance: RunEnvironmentSummary
  /** Non-null when a run executed outside PROD — never silent. */
  environmentWarning: string | null
  ship: ShipEvidence
  runs: RunEvidence
}

/**
 * Derive what the board should say, from durable evidence only.
 *
 * Invariants (the acceptance criteria, executable):
 *  - already Complete            -> no-change           (idempotent, re-runnable)
 *  - commits, not Complete       -> complete            (criterion 1)
 *  - no commits, no runs         -> no-change           (criterion 7: stays Planned)
 *  - no commits, runs present    -> no-change           (the engine owns that path)
 *  - absent run evidence         -> stated as absent    (criterion 4)
 *  - a non-PROD run              -> environmentWarning  (criterion 3)
 *  Nothing here ever re-runs work, and no number is ever estimated (criterion 6).
 */
export function deriveBoardSync(input: {
  story: BoardSyncStory
  ship: ShipEvidence
  runs: RunEvidence
  /** Injected for deterministic notes; ISO string. */
  now: string
  /**
   * The operator has explicitly decided to release a Hold/Deferred story whose
   * work shipped. Without this, a protected status is REPORTED and never written
   * — releasing a human park is a person's decision, expressed as a flag.
   */
  releaseHeld?: boolean
}): BoardSyncDecision {
  const { story, ship, runs, now } = input
  const provenance = summarizeRunEnvironments(runs.environments)

  const environmentWarning =
    provenance.nonProdEnvironments.length > 0
      ? `Forge run(s) executed outside PROD: ${provenance.nonProdEnvironments.join(', ')}. ` +
        'That evidence is not production evidence.'
      : null

  const base = { storyId: story.id, provenance, environmentWarning, ship, runs }

  if (story.status === 'Complete') {
    return { ...base, action: 'no-change', completion: null, reason: 'already-complete', note: null }
  }

  // A Hold/Deferred story was parked by a human. Shipped code does not repeal
  // that decision: report it as 'held-shipped' so the hold can be released on
  // purpose, rather than overwriting a deliberate state from a script.
  if (SYNC_PROTECTED_STATUSES.has(story.status) && ship.commits.length > 0) {
    if (!input.releaseHeld) {
      return { ...base, action: 'no-change', completion: null, reason: 'held-shipped', note: null }
    }
    return {
      ...base,
      action: 'complete',
      completion: 100,
      reason: 'released-held',
      note: buildSyncNote({
        storyId: story.id,
        ship,
        runs,
        provenance,
        environmentWarning,
        now,
        releasedFrom: story.status,
      }),
    }
  }

  if (ship.commits.length === 0) {
    const docsOnly = ship.docsOnly?.length ?? 0
    return {
      ...base,
      action: 'no-change',
      completion: null,
      // A packet commit is not shipped work — name that case explicitly rather
      // than reporting a generic "no evidence".
      reason:
        docsOnly > 0
          ? 'packet-only'
          : runs.runCount + runs.itemCount > 0
            ? 'run-evidence-only'
            : 'no-ship-evidence',
      note: null,
    }
  }

  return {
    ...base,
    action: 'complete',
    completion: 100,
    reason: 'shipped',
    note: buildSyncNote({ storyId: story.id, ship, runs, provenance, environmentWarning, now }),
  }
}

/**
 * The durable note. It names the commits that shipped the work and states
 * EXPLICITLY whether PROD holds run/work-item evidence for it — an absence is
 * recorded as an absence, never papered over with an estimate.
 */
export function buildSyncNote(input: {
  storyId: string
  ship: ShipEvidence
  runs: RunEvidence
  provenance: RunEnvironmentSummary
  environmentWarning: string | null
  now: string
  /** Set when the operator deliberately released a Hold/Deferred park. */
  releasedFrom?: string | null
}): string {
  const { storyId, ship, runs, provenance, environmentWarning, now, releasedFrom } = input
  const hasRunEvidence = runs.runCount > 0 || runs.itemCount > 0
  const evidenceLine = hasRunEvidence
    ? `PROD run evidence: PRESENT (${runs.runCount} run(s), ${runs.itemCount} work item(s)` +
      `${Object.keys(provenance.byEnvironment).length ? `; environments ${formatEnvironments(provenance)}` : ''}).`
    : `PROD run evidence: ABSENT (0 storyboard_story_run rows, 0 agent_work_item rows). ` +
      'Recorded as absent rather than replaced by a planning estimate.'

  return [
    `=== ENG-FORGE-SYNC-01 ship-time sync (${now}) ===`,
    `Shipped: ${ship.commits.length} commit(s) naming ${storyId} on the release branch.`,
    ...ship.commits.slice(0, 20).map((c) => `  - ${c}`),
    ship.docsOnly?.length
      ? `Excluded ${ship.docsOnly.length} docs/packet commit(s) — writing a story is not shipping it.`
      : null,
    releasedFrom
      ? `Hold released deliberately by the operator: this story was ${releasedFrom}. ` +
        'The status was a human park, not a lack of evidence.'
      : null,
    evidenceLine,
    environmentWarning ? `WARNING: ${environmentWarning}` : null,
    'Shipped code is the durable completion evidence. No work was re-executed to produce this',
    'result, and no number on this row was estimated.',
  ]
    .filter((line): line is string => line !== null)
    .join('\n')
}

function formatEnvironments(provenance: RunEnvironmentSummary): string {
  return Object.entries(provenance.byEnvironment)
    .map(([env, count]) => `${env}=${count}`)
    .join(' ')
}

/** Append a note without destroying existing history (matches the board convention). */
export function appendNote(existing: string | null | undefined, note: string): string {
  const current = (existing ?? '').trimEnd()
  return current.length > 0 ? `${current}\n\n${note}` : note
}


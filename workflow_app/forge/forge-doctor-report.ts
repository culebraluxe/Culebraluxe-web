// ---------------------------------------------------------------------------
// ENG-FORGE-DOCTOR-01 — the doctor's PURE renderer.
//
// `pnpm forge:doctor` answers one operator question: "is the control plane
// clear?" before a test run, instead of hand-writing the query every time.
//
// This module is the pure half: it takes a fully-resolved snapshot and renders
// the control-plane report AND the POSTCARD block. It has NO database access,
// NO filesystem access and NO clock of its own — every fact arrives as an
// input. That is what makes the empty-control-plane case, the
// board-drifted-from-table case and the worker-failing case unit tests instead
// of live probes.
//
// The command that gathers the facts lives in scripts/forge-doctor.ts, and is
// strictly READ-ONLY: it issues no update, no insert and no claim.
// ---------------------------------------------------------------------------

/**
 * The two ledgers a "claim" can live in. They are separate on purpose:
 * `forge_engine_task_execution` records engine role turns, while
 * `agent_work_item` enforces the system-wide single-active lock. They can
 * disagree, so the oldest-claim figure is NEVER printed without naming the
 * ledger it was read from.
 */
export type DoctorClaimLedger = 'forge_engine_task_execution' | 'agent_work_item'

export type DoctorOldestClaim = {
  ledger: DoctorClaimLedger
  /** Story id (engine ledger) or work-item id (agent_work ledger). */
  ref: string
  /** Age of the claim in milliseconds, measured from its claim/last-touch. */
  ageMs: number
}

export type DoctorControlPlane = {
  /** Stories the engine ledger has ever touched. */
  instances: number
  /** Engine ledger rows still non-terminal (claimed/running). */
  openTasks: number
  /** `agent_work_item` rows not in a terminal state. */
  openWorkItems: number
  /** Claims currently held, across BOTH ledgers. */
  activeClaims: number
  /** The oldest held claim, with its ledger named; null when nothing is held. */
  oldestClaim: DoctorOldestClaim | null
}

export type DoctorWorkerStatus = 'alive' | 'failing' | 'unknown'

export type DoctorWorkerLiveness = {
  status: DoctorWorkerStatus
  /** ISO of the newest recorded invocation, or null when none was read. */
  newestInvocationAt: string | null
  /** The most recent failure reason, or null. */
  lastFailure: string | null
  /** Invocations counted in the log, or null when the log was unreadable. */
  invocations: number | null
  /** The invocation log the reading came from. */
  logPath: string
}

export type DoctorPostcard = {
  /** Stories sitting in `Batched` on the board. */
  boardCount: number
  /** Stories the batch table says are staged. */
  tableCount: number
  activeDecisions: number
  roiWindowDays: number
  /** `describeRoiRow` output, one line per ROI row. */
  roiRows: string[]
  /** ISO of the newest learn-pass attempt, or null when none is recorded. */
  newestLearnPassAt: string | null
}

export type ForgeDoctorReportInput = {
  controlPlane: DoctorControlPlane
  worker: DoctorWorkerLiveness
  postcard: DoctorPostcard
  /** Injected ISO clock, so a render is deterministic and testable. */
  now: string
}

/**
 * Board-vs-table agreement, derived from the two counts alone.
 *
 * The board says N stories are Batched; the batch table says M are staged.
 * When they differ, the disagreement IS the bug (see scripts/forge-batch-status.ts),
 * so the doctor reports it as drift rather than averaging it away.
 */
export function deriveBoardVsTable(boardCount: number, tableCount: number): 'agree' | 'drift' {
  return boardCount === tableCount ? 'agree' : 'drift'
}

/** Human age: `3d 4h`, `4h 12m`, `7m`. Non-finite reads as `unknown`, never `0m`. */
export function formatAgeMs(ageMs: number): string {
  if (!Number.isFinite(ageMs) || ageMs < 0) return 'unknown'
  const totalMinutes = Math.floor(ageMs / 60_000)
  const days = Math.floor(totalMinutes / 1440)
  const hours = Math.floor((totalMinutes % 1440) / 60)
  const minutes = totalMinutes % 60
  if (days > 0) return `${days}d ${hours}h`
  if (hours > 0) return `${hours}h ${minutes}m`
  return `${minutes}m`
}

/** A control plane is clear when nothing is held and nothing is waiting. */
export function isControlPlaneClear(controlPlane: DoctorControlPlane): boolean {
  return controlPlane.openTasks === 0 && controlPlane.openWorkItems === 0 && controlPlane.activeClaims === 0
}

export function renderControlPlaneReport(controlPlane: DoctorControlPlane): string {
  const lines: string[] = []
  lines.push(`CONTROL PLANE: ${isControlPlaneClear(controlPlane) ? 'CLEAR' : 'BUSY'}`)
  lines.push(`  instances: ${controlPlane.instances}`)
  lines.push(`  open engine tasks: ${controlPlane.openTasks}`)
  lines.push(`  open work items: ${controlPlane.openWorkItems}`)
  lines.push(`  active claims: ${controlPlane.activeClaims}`)
  if (controlPlane.oldestClaim) {
    const claim = controlPlane.oldestClaim
    // The ledger is named, not implied: the two ledgers can disagree.
    lines.push(`  oldest claim: ${formatAgeMs(claim.ageMs)} — ${claim.ledger} (${claim.ref})`)
  } else {
    lines.push('  oldest claim: none')
  }
  return lines.join('\n')
}

export function renderWorkerLiveness(worker: DoctorWorkerLiveness): string {
  const lines: string[] = []
  lines.push(`WORKER: ${worker.status.toUpperCase()}`)
  lines.push(`  log: ${worker.logPath}`)
  lines.push(`  newest invocation: ${worker.newestInvocationAt ?? 'never'}`)
  lines.push(`  invocations: ${worker.invocations === null ? 'unknown' : worker.invocations}`)
  lines.push(`  last failure: ${worker.lastFailure ?? 'none'}`)
  return lines.join('\n')
}

export function renderPostcard(postcard: DoctorPostcard): string {
  const agreement = deriveBoardVsTable(postcard.boardCount, postcard.tableCount)
  const lines: string[] = []
  lines.push('POSTCARD')
  lines.push(
    `  board vs table: ${agreement === 'agree' ? 'agree' : 'DRIFT — this is a bug, report it'}` +
      ` (board ${postcard.boardCount} vs table ${postcard.tableCount})`,
  )
  lines.push(`  active decisions: ${postcard.activeDecisions}`)
  lines.push(`  ROI (last ${postcard.roiWindowDays} day(s)):`)
  if (postcard.roiRows.length === 0) {
    lines.push('    (no ROI rows in the window)')
  } else {
    for (const row of postcard.roiRows) lines.push(`    - ${row}`)
  }
  lines.push(`  newest learn-pass attempt: ${postcard.newestLearnPassAt ?? 'never'}`)
  return lines.join('\n')
}

/**
 * The whole report: control plane, worker liveness, then the POSTCARD block.
 * Pure composition — the same inputs always render the same string.
 */
export function renderForgeDoctorReport(input: ForgeDoctorReportInput): string {
  return [
    `FORGE DOCTOR — ${input.now}`,
    '',
    renderControlPlaneReport(input.controlPlane),
    '',
    renderWorkerLiveness(input.worker),
    '',
    renderPostcard(input.postcard),
  ].join('\n')
}

// ---------------------------------------------------------------------------
// FORGE ROI — the thin session rollup (ENG-FORGE-FACTORY-01 Phase 4, Object 4).
//
// THE POINT IS TO STOP TOKEN-MAXXING THE NIGHT BATCH: "last 7 days, count and cost by kind" is enough to
// see that the cheap policy is doing the volume work and the judgment policy is being spent deliberately.
// It is explicitly NOT a finance system, and the packet says so twice.
//
// THE ONE RULE THAT MATTERS HERE: **widgets are not dollars.** `cost_widgets` is Forge's standardized,
// model-relative consumption quantity (model_weight × elapsed minutes), and there is deliberately no
// widgets→USD rate in this system. `cost_usd` means vendor-reported actual and is often null. So this
// module never emits a currency figure, and its own summary says which unit it is in - because the fastest
// way to turn a cheap signal into a wrong decision is to render "1,240" next to a "$".
//
// WHAT IS ALREADY PERSISTED, and therefore not rebuilt here (the packet: "reuse ENG-FORGE-COST-01 /
// run-usage surfaces; do not invent a second meter"):
//   - kind + model_policy: `agent_work_item` (migration 179, Phase 1)
//   - result_status + cost_widgets + model_used: `storyboard_story_run` (migrations 133/107)
//   - wall time: `agent_work_item.started_at` / `finished_at`
// So Phase 4 is a READ MODEL and a strip. It adds no column, no meter and no table.
// ---------------------------------------------------------------------------

export type RoiAttempt = {
  /** `agent_work_item.kind`; null on items queued before migration 179. */
  kind: string | null
  modelPolicy: string | null
  /** Work item state: Done | Error | Cancelled | ... */
  state: string
  startedAt: string | null
  finishedAt: string | null
  /** The run's own verdict, when the item is linked to one. */
  resultStatus: string | null
  costWidgets: number | null
}

export type RoiRow = {
  kind: string
  policy: string
  attempts: number
  completed: number
  failed: number
  /** Mean wall time in minutes over the attempts where both ends are known. */
  meanWallMinutes: number | null
  /** How many attempts contributed to that mean. */
  wallMinutesKnown: number
  /** Sum of the known widget costs. NOT money. */
  costWidgets: number
  /** How many attempts reported a cost at all. */
  costKnown: number
}

export type RoiSummary = {
  windowDays: number
  unit: string
  rows: RoiRow[]
  totals: { attempts: number; completed: number; failed: number; costWidgets: number }
  /** Honest coverage, because a rollup over 3 of 20 attempts must not read as the whole story. */
  coverage: { attempts: number; costKnown: number; wallTimeKnown: number }
  note: string
}

/** The unit label, in one place, so no caller can invent a currency without changing this. */
export const ROI_UNIT = 'widgets (model weight × minutes — not dollars)'

export const ROI_UNRECORDED_KIND = 'unrecorded'

function minutesBetween(startIso: string | null, endIso: string | null): number | null {
  if (!startIso || !endIso) return null
  const start = Date.parse(startIso)
  const end = Date.parse(endIso)
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return null
  return (end - start) / 60_000
}

/**
 * The rollup. Pure, so the acceptance fixture is a unit test rather than a screenshot.
 *
 * An attempt with no kind lands in an `unrecorded` bucket instead of being dropped: those are real runs
 * from before Phase 1, and a rollup that silently excludes them is how a coverage gap becomes invisible.
 */
export function summarizeRoi(attempts: readonly RoiAttempt[], windowDays = 7): RoiSummary {
  const buckets = new Map<
    string,
    { kind: string; policy: string; rows: RoiAttempt[]; wallTotal: number; wallKnown: number }
  >()

  for (const attempt of attempts) {
    const kind = (attempt.kind ?? '').trim() || ROI_UNRECORDED_KIND
    const policy = (attempt.modelPolicy ?? '').trim() || ROI_UNRECORDED_KIND
    const key = `${kind}::${policy}`
    const bucket = buckets.get(key) ?? { kind, policy, rows: [], wallTotal: 0, wallKnown: 0 }
    bucket.rows.push(attempt)
    const wall = minutesBetween(attempt.startedAt, attempt.finishedAt)
    if (wall !== null) {
      bucket.wallTotal += wall
      bucket.wallKnown += 1
    }
    buckets.set(key, bucket)
  }

  const rows: RoiRow[] = [...buckets.values()]
    .map((bucket) => ({
      kind: bucket.kind,
      policy: bucket.policy,
      attempts: bucket.rows.length,
      completed: bucket.rows.filter((row) => row.state === 'Done').length,
      failed: bucket.rows.filter((row) => row.state === 'Error').length,
      meanWallMinutes: bucket.wallKnown > 0 ? Math.round((bucket.wallTotal / bucket.wallKnown) * 10) / 10 : null,
      wallMinutesKnown: bucket.wallKnown,
      costWidgets: Math.round(bucket.rows.reduce((sum, row) => sum + (row.costWidgets ?? 0), 0) * 100) / 100,
      costKnown: bucket.rows.filter((row) => row.costWidgets !== null && row.costWidgets !== undefined).length,
    }))
    // Most attempts first, then kind, then policy: deterministic, so two readers see one order.
    .sort((a, b) => b.attempts - a.attempts || a.kind.localeCompare(b.kind) || a.policy.localeCompare(b.policy))

  return {
    windowDays,
    unit: ROI_UNIT,
    rows,
    totals: {
      attempts: attempts.length,
      completed: attempts.filter((attempt) => attempt.state === 'Done').length,
      failed: attempts.filter((attempt) => attempt.state === 'Error').length,
      costWidgets: Math.round(rows.reduce((sum, row) => sum + row.costWidgets, 0) * 100) / 100,
    },
    coverage: {
      attempts: attempts.length,
      costKnown: attempts.filter(
        (attempt) => attempt.costWidgets !== null && attempt.costWidgets !== undefined,
      ).length,
      wallTimeKnown: attempts.filter(
        (attempt) => minutesBetween(attempt.startedAt, attempt.finishedAt) !== null,
      ).length,
    },
    note:
      'Widgets are Forge consumption units, not currency. There is no widgets-to-dollars rate in this ' +
      'system, and cost_usd stays reserved for vendor-reported actuals.',
  }
}

/** One line per row, for a terminal or a log: `fix/cheap · 12 attempts · 8 done · 480 widgets`. */
export function describeRoiRow(row: RoiRow): string {
  const wall = row.meanWallMinutes === null ? 'wall n/a' : `${row.meanWallMinutes}m mean`
  return (
    `${row.kind}/${row.policy} · ${row.attempts} attempt(s) · ${row.completed} done` +
    `${row.failed ? ` · ${row.failed} failed` : ''} · ${wall} · ${row.costWidgets} widgets` +
    `${row.costKnown < row.attempts ? ` (cost on ${row.costKnown})` : ''}`
  )
}

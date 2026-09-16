import { engineSql } from '../engine-client'
import type { ForgeGateEvidence } from './forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 #1 — durable gate evidence reader (maps the Neon run tables).
//
// The engine instance variables carry workType (set at start). Everything else
// a decision gate needs (qaPassed, publishSucceeded, deploymentSucceeded,
// productionVerified, migrationRequired, ...) is the OUTPUT of a role run that
// the Forge runtime records as a storyboard_story_run row (run_type +
// result_status [+ commit_hash]) and an agent_work_item (role/state/attempts).
//
// This reader maps those DURABLE rows back to ForgeGateEvidence so readFacts is
// stable across separate engine calls/resumes — no in-memory evidence. Mapping
// contract (run_type -> gate fact), decided by the NEWEST row of each family:
//   qa|assay complete/success  -> qaPassed true
//   publish complete           -> publishSucceeded true
//   deploy complete            -> deploymentSucceeded true
//   smoke|production complete  -> productionVerified true
//   migration (any) complete   -> migrationRequired true + verified
//
// The real role runner (#5) records these rows; this is the reader half of the
// durable-evidence path. DB-free callers inject nothing; it queries the shared
// engine/app database via engineSql().
// ---------------------------------------------------------------------------

function isClean(status: string | null | undefined): boolean {
  // WORD-ANCHORED on purpose. The old `/complete|success|pass/i` matched INSIDE a word, so a status of
  // `Incomplete` read as a pass — the exact opposite of what it says. Anchored, `Complete`/`Completed`/
  // `success`/`passed` still match and `Incomplete` cannot.
  return /\b(?:complete|success|pass)/i.test(status ?? '')
}

/** The gate facts this module derives, one family per fact. */
type ForgeFactFamily = 'qa' | 'publish' | 'deploy' | 'smoke' | 'migration'

function forgeFactFamily(lowerRunType: string): ForgeFactFamily | null {
  if (lowerRunType.includes('qa') || lowerRunType.includes('assay')) return 'qa'
  if (lowerRunType.includes('publish')) return 'publish'
  if (lowerRunType.includes('deploy')) return 'deploy'
  if (lowerRunType.includes('smoke') || lowerRunType.includes('production')) return 'smoke'
  if (lowerRunType.includes('migrat')) return 'migration'
  return null
}

/** Assay failure codes that mean a CONFIG/verification gap, NOT a candidate defect
 * (assay-evidence.ts evaluateAssayEvidence). A gap can never be fixed by repairing
 * the candidate -> it must route to HOLD, not smith (the FINAL-02 deadlock).
 *
 * RETIRED as a READER rule (Captain, 2026-09-16 — "one verdict, one vocabulary").
 *
 * This Set let this module RE-DERIVE a QA verdict from a failure-code vocabulary that belongs to the
 * Assay lane, so two places spoke about one measurement in two languages — and on 2026-09-16 that
 * produced a QA that failed with CANDIDATE_MISMATCH, which is in neither vocabulary, so it fell through
 * to `qaPassed: false` while a stale row said otherwise. The gap is NOT re-derived here any more.
 *
 * The gap distinction still exists, authored by the lane that measures it: `collectAssayEvidence` sets
 * `verificationGap` on the LIVE evidence, and the router reads it there (forge-facts -> routeQaResult).
 * On a fresh read after a restart, a non-pass QA row is simply NOT A PASS — and a QA failure with no
 * classification disposition already HOLDs, so nothing is lost by refusing to guess the reason.
 */

export type ForgeRunRowShape = {
  run_type: string | null
  result_status: string | null
  commit_hash: string | null
  failure_code?: string | null
}

/**
 * ONE FACT, ONE MEASUREMENT — AND THE MEASUREMENT IS THE NEWEST.
 *
 * This mapper used to OR the whole history: `if (clean) evidence.qaPassed = true` for ANY qa/assay row, and
 * the same for publish/deploy/smoke. Because the read spans the entire story (newest 50 runs, no generation
 * filter) and a story's runs OUTLIVE an engine reset, an OLD pass certified every later generation.
 *
 * Measured live on 2026-09-16: `ENG-QA-SINGLE-VERDICT-01` carried ten clean `assay` runs from earlier that
 * morning, tonight's assay held on `CANDIDATE_MISMATCH`, and the router read `qaPassed: true` — then
 * advanced to `deploy` on a candidate no lane had verified. Reproduced against the real rows: this function
 * returned `{"qaPassed":true}` while the newest qa/assay row read `Hold`. That is AGENTS.md's rule — a cached
 * copy must never outvote the row — broken *inside the reader*: QA's verdict for a generation is the NEWEST
 * measurement of it, never the best one the story ever had. `deploymentSucceeded` had the same shape: once
 * any deployment was clean, a story could never stop being deployed.
 *
 * So the FIRST row seen for a family decides it, and the caller supplies runs NEWEST-FIRST (the SQL below
 * orders by `started_at desc nulls last, created_at desc`). A family whose newest row is not clean reports
 * `false` — never "unknown", because a fact that was measured and failed is not unmeasured.
 */
export function mapRunsToGateEvidence(rows: ForgeRunRowShape[]): ForgeGateEvidence {
  const evidence: ForgeGateEvidence = {}
  const decided = new Set<ForgeFactFamily>()

  for (const r of rows) {
    const rt = (r.run_type ?? '').toLowerCase()
    if (!rt) continue
    const family = forgeFactFamily(rt)
    if (!family || decided.has(family)) continue
    decided.add(family)
    const clean = isClean(r.result_status)
    switch (family) {
      case 'qa':
        evidence.qaPassed = clean
        break
      case 'publish':
        evidence.publishSucceeded = clean
        break
      case 'deploy':
        evidence.deploymentSucceeded = clean
        break
      case 'smoke':
        evidence.productionVerified = clean
        break
      case 'migration':
        // A migration run declares the story's REQUIREMENT, so only a clean one sets it — the meaning the
        // old reader had, now tied to the newest measurement instead of to any measurement.
        evidence.migrationRequired = clean
        break
    }
  }

  return evidence
}

export async function readStoryGateEvidence(
  storyId: string,
): Promise<ForgeGateEvidence> {
  const rows = (await engineSql()`
    select run_type, result_status, commit_hash, failure_code
    from storyboard_story_run
    where story_id = ${storyId}
    order by started_at desc nulls last, created_at desc
    limit 50
  `) as ForgeRunRowShape[]
  const evidence = mapRunsToGateEvidence(rows)

  // V11-S1: merge the durable repair/replan observers + last QA disposition from
  // the canonical story row. These are OBSERVERS (evidence for the engine to read
  // as facts), not a worker-owned stop condition — the engine graph routes to
  // HOLD when a budget is exhausted.
  const story = (
    await engineSql()`
      select forge_repair_attempts, forge_replan_attempts, forge_last_qa_disposition
      from storyboard_story
      where id = ${storyId}
    `
  )[0] as
    | {
        forge_repair_attempts: number | null
        forge_replan_attempts: number | null
        forge_last_qa_disposition: string | null
      }
    | undefined
  if (story) {
    evidence.repairAttempts = Number(story.forge_repair_attempts ?? 0)
    evidence.replanAttempts = Number(story.forge_replan_attempts ?? 0)
    if (story.forge_last_qa_disposition) {
      evidence.disposition = story.forge_last_qa_disposition as ForgeGateEvidence['disposition']
    }
  }
  return evidence
}

/**
 * The durable evidence reader wired as ApplicationPort.readFacts: query the run
 * table for a story and project its gate evidence.
 */
export function createStoryGateEvidenceReader(): (storyId: string) => Promise<ForgeGateEvidence> {
  return readStoryGateEvidence
}

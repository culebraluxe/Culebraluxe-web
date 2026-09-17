// ---------------------------------------------------------------------------
// LEARN LOOP — the unattended pass that turns traces into work (Phase 3, Object 3).
//
// WHERE IT RUNS: at the top of `scripts/agent-work-entry.ts`, right after `fireDueForgeBatches`, so it
// rides the existing three-minute launchd tick. There is no second daemon and no cron entry to forget.
//
// WHAT IT DOES, in the packet's order:
//   1. run the silent-failure hunter over the files that changed inside the window, and ask for the stale
//      claims (an abandoned claim is the P0 the factory can already detect deterministically);
//   2. file AT MOST ONE learn item, skipping any pattern that already has an open one;
//   3. write the story, the packet (from the template) and the row, then advance the window anchor.
//
// WHAT IT DELIBERATELY DOES NOT DO: it does not fix anything, does not commit, does not merge, does not
// promote a decision, and does not write an acceptance criterion. The item it files goes to Lead and
// Architect like any other story; a machine that files work and also decides the fix is what the packet's
// stop conditions exist to prevent.
//
// THE WINDOW: "since the last successful learn run, max 24h". The anchor lives in `.forge-context/`
// (gitignored runtime state), so a pass that files nothing still advances it and the same change is not
// re-reported every three minutes forever.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { gitBinary } from '../lib/worker-workspace/provisioner'

import { listStaleAgentWork } from '../db/agent-work'
import { getStagingBatch } from '../db/forge-batch'
import { listOpenLearnPatternKeys, openReadyLearnItem, stageLearnStory } from '../db/forge-learn'
import { createStoryboardStory, getStoryboardStory, setStoryboardStatus } from '../db/storyboard'
import { writeIfChanged } from '../lib/artifact-file'
import {
  DEFAULT_LEARN_WINDOW_HOURS,
  LEARN_PATTERN_SEVERITY,
  buildLearnItemInstructions,
  decideLearnItem,
  isLearnPattern,
  learnPatternKey,
  learnStoryId,
  learnWindowStart,
  parseLearnAnchor,
  renderLearnPacket,
  type LearnAnchor,
  type LearnCandidate,
} from '../lib/forge-learn'
import { findSilentFailures } from './silent-failure-patterns'

/** Code the hunter can actually read. Docs and generated output are not sources of silent failures. */
const CODE_EXTENSIONS = /\.(ts|tsx|mjs|js)$/
const SKIP_PREFIXES = ['docs/', 'node_modules/', '.next/', '.vercel/', '.forge/', 'testv2/']

/**
 * Files the learn loop must not learn from, with the reason for each:
 *
 *   - TEST FILES contain the patterns on purpose. `newSilentFailures` is proven by fixtures that assert
 *     "this IS a swallowed catch", so scanning them files a learn item about our own test data.
 *   - `agent-runtime/silent-failure-patterns.ts` DESCRIBES all four patterns as regexes. The hunter
 *     matches its own source (`status:\s*500` inside a pattern literal reads as a bare 500), which is a
 *     false positive about the detector, not a finding about the product.
 *
 * Measured on the first dry run, 2026-09-15: 9 candidates, and 5 of them were these two classes. Filing
 * them is how a learn loop becomes noise that everyone mutes - the same calibration that took packet-lint
 * rule 10 from 16 hits to 3 (15 were the packets' own shorthand).
 */
const SKIP_FILES = [/\.(test|spec)\.(ts|tsx|mjs|js)$/, /^agent-runtime\/silent-failure-patterns\.ts$/]

/** Would the learn loop look at this path? One place, so a probe and the pass cannot disagree. */
export function isLearnWindowPath(path: string): boolean {
  if (!CODE_EXTENSIONS.test(path)) return false
  if (SKIP_PREFIXES.some((prefix) => path.startsWith(prefix))) return false
  if (SKIP_FILES.some((pattern) => pattern.test(path))) return false
  return true
}

/** Bounded on purpose: a 500-file window would spend the pass reading instead of doing. */
const MAX_FILES_PER_WINDOW = 40
const MAX_EVIDENCE_PER_CANDIDATE = 5

export type LearnFile = { path: string; content: string; firstSeen: string; lastSeen: string }

export function learnAnchorPath(root: string): string {
  return join(root, '.forge-context', 'learn-last-run.json')
}

export function readLearnAnchor(root: string): LearnAnchor | null {
  const path = learnAnchorPath(root)
  if (!existsSync(path)) return null
  return parseLearnAnchor(readFileSync(path, 'utf8'))
}

function writeLearnAnchor(root: string, anchor: LearnAnchor): void {
  writeIfChanged(learnAnchorPath(root), `${JSON.stringify(anchor, null, 2)}\n`)
}

/**
 * The files that changed in the window, newest first, with their first and last touch.
 *
 * One `git log` pass gives both dates: commits arrive newest-first, so the first hit for a path is its
 * last touch and the last hit is its first touch inside the window.
 */
export function changedFilesInWindow(root: string, sinceIso: string, limit = MAX_FILES_PER_WINDOW): LearnFile[] {
  let raw = ''
  try {
    raw = execFileSync(gitBinary(), ['log', `--since=${sinceIso}`, '--name-only', '--pretty=format:__C__%cI'], {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 16 * 1024 * 1024,
    })
  } catch {
    return []
  }

  const order: string[] = []
  const meta = new Map<string, { firstSeen: string; lastSeen: string }>()
  let commitDate = ''
  for (const line of raw.split('\n')) {
    if (line.startsWith('__C__')) {
      commitDate = line.slice(5).trim()
      continue
    }
    const path = line.trim()
    if (!path || !commitDate) continue
    if (!isLearnWindowPath(path)) continue
    const existing = meta.get(path)
    if (!existing) {
      order.push(path)
      meta.set(path, { firstSeen: commitDate, lastSeen: commitDate })
    } else {
      existing.firstSeen = commitDate
    }
  }

  const files: LearnFile[] = []
  for (const path of order.slice(0, limit)) {
    const full = join(root, path)
    if (!existsSync(full)) continue
    files.push({ path, content: readFileSync(full, 'utf8'), ...meta.get(path)! })
  }
  return files
}

/**
 * Turn hunter hits into candidates, ONE PER PATTERN+PATH.
 *
 * Per path rather than per pattern, because a single `empty-catch` item that says "somewhere" would
 * silence every other file for the window and the second real defect would hide behind the first ticket.
 */
export function learnCandidatesFromFiles(files: readonly LearnFile[]): LearnCandidate[] {
  const byKey = new Map<string, { candidate: LearnCandidate; lines: number[] }>()
  for (const hit of findSilentFailures(files.map((file) => ({ path: file.path, content: file.content })))) {
    // The empty-success pattern BLOCKS at the gate; it is not a nightly note. Only the report-only
    // patterns file, so a blocked shape is not also queued as a story beside the block.
    if (!isLearnPattern(hit.pattern)) continue
    const file = files.find((candidate) => candidate.path === hit.path)
    const key = learnPatternKey(hit.pattern, hit.path)
    const existing = byKey.get(key)
    if (existing) {
      existing.candidate.hitCount += 1
      existing.lines.push(hit.line)
      if (file?.firstSeen) existing.candidate.firstSeen = existing.candidate.firstSeen || file.firstSeen
      existing.candidate.lastSeen = file?.lastSeen ?? existing.candidate.lastSeen
      continue
    }
    byKey.set(key, {
      candidate: {
        pattern: hit.pattern,
        key,
        severity: LEARN_PATTERN_SEVERITY[hit.pattern],
        title: `${hit.pattern} in ${hit.path}`,
        evidence: [`${hit.path}:${hit.line}`],
        hitCount: 1,
        firstSeen: file?.firstSeen ?? '',
        lastSeen: file?.lastSeen ?? '',
      },
      lines: [hit.line],
    })
  }

  return [...byKey.values()].map(({ candidate, lines }) => ({
    ...candidate,
    evidence: [
      ...candidate.evidence,
      ...lines.slice(1, MAX_EVIDENCE_PER_CANDIDATE).map((line) => `${candidate.pattern} at line ${line}`),
    ].slice(0, MAX_EVIDENCE_PER_CANDIDATE),
  }))
}

/** The P0 the factory already detects: work claimed and then abandoned. */
export function staleClaimCandidate(
  items: ReadonlyArray<{ id: string; updatedAt: string }>,
  windowEndIso: string,
): LearnCandidate | null {
  const usable = items.filter((item) => item.updatedAt)
  if (usable.length === 0) return null
  const stamps = usable.map((item) => item.updatedAt).sort()
  return {
    pattern: 'stale-claim',
    key: 'stale-claim',
    severity: LEARN_PATTERN_SEVERITY['stale-claim'],
    title: `${usable.length} abandoned work claim(s)`,
    evidence: usable.map((item) => `agent_work_item:${item.id}`),
    hitCount: usable.length,
    firstSeen: stamps[0],
    lastSeen: stamps[stamps.length - 1] || windowEndIso,
  }
}

export type LearnPassResult = {
  windowStart: string
  windowEnd: string
  filesScanned: number
  candidates: number
  applied: boolean
  /**
   * The item the pass WOULD file. Present in a dry run too, because "filed nothing" next to a list of
   * candidates is a report that hides the one thing the reader asked for.
   */
  wouldFile: { key: string; severity: string; hitCount: number } | null
  filed: {
    storyId: string
    key: string
    severity: string
    via: 'staging' | 'ready'
    batchId: string | null
    packetWritten: boolean
  } | null
  deferred: string[]
  skipped: string[]
}

/**
 * ONE PASS. Gather, decide, file at most one, advance the anchor.
 *
 * The anchor advances on a pass that files NOTHING too, and that is deliberate: the window means "changes
 * since we last looked", so without it every pass would re-report the same commit forever. The cost is
 * stated rather than hidden — a burst of ten findings files one and reports the other nine as `deferred`.
 */
export async function runLearnPass(input: {
  root: string
  now?: Date
  windowHours?: number
  apply: boolean
  staleAfterMinutes?: number
}): Promise<LearnPassResult> {
  const now = input.now ?? new Date()
  const windowStart = learnWindowStart(
    readLearnAnchor(input.root),
    now,
    input.windowHours ?? DEFAULT_LEARN_WINDOW_HOURS,
  )
  const windowEndIso = now.toISOString()
  const files = changedFilesInWindow(input.root, windowStart.toISOString())
  const stale = await listStaleAgentWork(input.staleAfterMinutes ?? 60)
  const candidates = [
    ...learnCandidatesFromFiles(files),
    ...[staleClaimCandidate(stale, windowEndIso)].filter((candidate): candidate is LearnCandidate => Boolean(candidate)),
  ]

  const decision = decideLearnItem({
    candidates,
    openPatternKeys: new Set(await listOpenLearnPatternKeys()),
  })

  const result: LearnPassResult = {
    windowStart: windowStart.toISOString(),
    windowEnd: windowEndIso,
    filesScanned: files.length,
    candidates: candidates.length,
    applied: input.apply,
    wouldFile: decision.filed
      ? { key: decision.filed.key, severity: decision.filed.severity, hitCount: decision.filed.hitCount }
      : null,
    filed: null,
    deferred: decision.deferred.map((candidate) => candidate.key),
    skipped: decision.skipped.map((candidate) => candidate.key),
  }
  if (!input.apply) return result

  const filed = decision.filed
  if (filed) {
    const storyId = learnStoryId(filed.key, windowEndIso)
    const instructions = buildLearnItemInstructions(filed)
    // The P0 exception from the packet: bypass staging only when staging is empty, so a live batch is
    // never disturbed by a priority finding.
    const staging = await getStagingBatch()
    const via: 'staging' | 'ready' = filed.severity === 'P0' && !staging ? 'ready' : 'staging'

    if (!(await getStoryboardStory(storyId))) {
      await createStoryboardStory({
        id: storyId,
        workstream: 'ENGINEERING',
        title: `learn: ${filed.key}`,
        priority: filed.severity === 'P0' ? 'High' : 'Medium',
        status: 'Planned',
        notes: instructions,
        batch: null,
        goal: `Verify and resolve ${filed.key}`,
        scope: null,
        dependencies: null,
        preconditions: null,
        architectBrief: null,
        contextRefs: null,
        acceptanceCriteria: null,
        postconditions: null,
        completion: 0,
        rollup: true,
        plannedStartAt: null,
        actualStartAt: null,
        completedAt: null,
      })
    }

    // A GIT FILE ONLY ON A learn/* BRANCH (Grok, 2026-09-16). Committing the packet on `main` closed the
    // dirty-file race and opened an origin-diverge one: the next `git pull --ff-only` dies the moment origin's
    // main has moved, and a mailbox commit is enough to move it. So on main this pass writes NO git file — the
    // row and the story are already durable, and the template travels in the story notes until the Architect
    // or a human packets it. Warn-and-leave-dirty is the old bug with a log line, so we do not do that either.
    const learnBranch = (() => {
      try {
        return execFileSync(gitBinary(), ['rev-parse', '--abbrev-ref', 'HEAD'], {
          cwd: input.root,
          encoding: 'utf8',
        }).trim()
      } catch {
        return ''
      }
    })()
    const onLearnBranch = learnBranch.startsWith('learn/')
    if (input.apply && !onLearnBranch) {
      console.warn(
        `learn: no packet file written (branch '${learnBranch || 'unknown'}' is not learn/*); ` +
          'the story row carries the learn',
      )
    }
    const packetWritten =
      input.apply && onLearnBranch
        ? writeIfChanged(
            join(input.root, 'docs/agent/packets', `${storyId}.md`),
            renderLearnPacket(filed, {
              storyId,
              windowStart: windowStart.toISOString(),
              windowEnd: windowEndIso,
            }),
          )
        : false
    // D (Grok, 2026-09-15): this file lands on the PRIMARY checkout, and the unattended worker's next tick
    // opens with `git pull --ff-only`, which dies on a dirty tree. Observed live: three
    // LEARN-SWALLOWED-CATCH-*.md packets sitting untracked, waiting to block a pull. The pass that writes the
    // file also COMMITS it — current branch, NO push: a learn packet is history, not a release.
    if (input.apply && packetWritten) {
      const relative = join('docs/agent/packets', `${storyId}.md`)
      try {
        execFileSync(gitBinary(), ['add', '--', relative], { cwd: input.root, stdio: 'ignore' })
        execFileSync(gitBinary(), ['commit', '-q', '-m', `learn: packet ${storyId}`, '--', relative], {
          cwd: input.root,
          stdio: 'ignore',
        })
      } catch (err) {
        // Never fail the pass for this: the row and the story are already durable, and a packet left dirty is
        // visible on the next tick's git-sync. The warning lands in the worker log, where an unattended
        // failure has to be legible.
        console.warn(`learn: packet not committed (${relative}): ${(err as Error).message}`)
      }
    }

    let batchId: string | null = null
    if (via === 'ready') {
      await openReadyLearnItem({ storyId, patternKey: filed.key, instructions })
    } else {
      await setStoryboardStatus(storyId, 'Batched')
      const staged = await stageLearnStory({ storyId, patternKey: filed.key })
      batchId = staged.batchId
    }

    result.filed = { storyId, key: filed.key, severity: filed.severity, via, batchId, packetWritten }
  }

  writeLearnAnchor(input.root, { at: windowEndIso, lastKey: filed?.key ?? null })
  return result
}

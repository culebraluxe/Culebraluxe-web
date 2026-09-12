import { createAgentRuntimeForgeRoleRunner } from '../workflow_app/forge/agent-runtime-role-runner'
import { driveForgeStory } from '../workflow_app/forge/forge-executor'
import { assertForgeLaneMayStart } from '../workflow_app/forge/forge-execution-target'
import {
  detectForgeDualWrite,
  parseForgeRoutingBrain,
} from '../workflow_app/forge/forge-routing-brain'
import { findActiveForgeInstance } from '../workflow_app/forge/forge-engine-runtime'
import { getStoryboardStory } from '../db/storyboard'
import { markForgeStoryHumanHold } from '../db/forge-story-state'
import { storyReadyToRunReasons } from '../workflow_app/forge/forge-ready-gate'

const args = process.argv.slice(2)
const value = (flag: string): string | undefined => {
  const index = args.indexOf(flag)
  return index >= 0 ? args[index + 1] : undefined
}

/** Map a --until value to a ForgeStopTarget: named role, or an exact engine node. */
function parseUntil(raw: string | undefined): { role: 'scout' | 'architect' | 'lead' } | { node: string } | undefined {
  if (!raw) return undefined
  if (raw === 'scout' || raw === 'architect' || raw === 'lead') return { role: raw }
  if (raw.startsWith('node:')) return { node: raw.slice(5) }
  return { node: raw }
}

async function main(): Promise<void> {
  const storyId = value('--story')
  const workType = value('--work-type') ?? 'FEATURE'
  if (!storyId) {
    throw new Error(
      'usage: forge-engine-worker --story <story-id> [--work-type FEATURE|BUG|HOTFIX|RESEARCH|MIGRATION] [--until scout|architect|lead|node:<engine-node-id>]',
    )
  }
  if (!['FEATURE', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION'].includes(workType)) {
    throw new Error(`invalid --work-type ${JSON.stringify(workType)}`)
  }

  // ENG-FORGE-SYNC-GUARD-01 — fail closed BEFORE the ready gate, before any board
  // read and before a work item exists.
  //
  // This script is the entry point that made "DEV keeps getting Forge data" real:
  // it launched with `process.env.EXECUTION_ENV ?? 'DEV'`, so a bare
  // `pnpm forge:engine --story X` ran the lane as DEV, and with APP_ENV unset the
  // control-plane rows also resolved to the DEV database. Both halves are now
  // asserted here, and `pnpm forge:engine` sets both variables so the normal path
  // is PROD rather than merely permitted to be.
  const laneTarget = assertForgeLaneMayStart({ env: process.env })

  const brain = parseForgeRoutingBrain()
  const engineActive = Boolean(await findActiveForgeInstance(storyId))
  const dual = detectForgeDualWrite({
    storyId,
    reducerTouched: brain === 'reducer' && engineActive,
    engineInstanceActive: engineActive && brain === 'engine',
  })
  if (!dual.ok) {
    throw new Error(`Forge dual-write refused for story ${storyId}`)
  }

  // Ready gate: a QA-applicable story must NOT leave Planned (start a fresh
  // engine instance and burn Scout/Architect/Lead/Smith) when its packet cannot
  // be assayed. Resume of an already-active instance is never blocked.
  if (!engineActive && workType !== 'RESEARCH' && workType !== 'MIGRATION') {
    const story = await getStoryboardStory(storyId)
    const reasons = storyReadyToRunReasons({
      workType,
      acceptanceCriteria: story?.acceptanceCriteria ?? null,
      assayCommands: story?.assayCommands ?? null,
    })
    if (reasons.length > 0) {
      await markForgeStoryHumanHold(
        storyId,
        `ready-gate: story must not start without an assayable contract: ${reasons.join('; ')}`,
      )
      console.log(JSON.stringify({ brain, readyGate: 'HOLD', storyId, reasons }, null, 2))
      return
    }
  }

  const workerId = process.env.AGENT_WORKER_ID?.trim() || `forge-engine-${process.pid}`
  const stopAfter = parseUntil(value('--until'))
  const stamp = (): string => new Date().toISOString().slice(11, 19)
  const result = await driveForgeStory(storyId, {
    start: { workType: workType as 'FEATURE' | 'BUG' | 'HOTFIX' | 'RESEARCH' | 'MIGRATION' },
    runner: createAgentRuntimeForgeRoleRunner({
      workerId,
      executionEnvironment: laneTarget,
    }),
    workerId,
    // SPLIT concurrency: default 2 (the lane is enabled by default now).
    splitConcurrency: Math.max(1, Math.trunc(Number(process.env.FORGE_SPLIT_CONCURRENCY ?? '2')) || 2),
    onProgress: (message) => console.log(`[${stamp()}] ${message}`),
    ...(stopAfter ? { stopAfter } : {}),
  })
  console.log(JSON.stringify({ brain, ...result }, null, 2))
  if (result.exhausted && !result.stoppedAfter) process.exitCode = 2
}

main().catch((error) => {
  console.error(String((error as Error)?.stack ?? error))
  process.exitCode = 1
})

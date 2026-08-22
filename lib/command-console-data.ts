// ---------------------------------------------------------------------------
// SDLC Command Console read model (thin UI over ENG-18).
//
// Assembles the cockpit snapshot from the REAL control-plane tables only:
//   storyboard_story         (authoritative backlog/spec)
//   agent_work_item          (durable command/queue)
//   storyboard_story_run     (execution evidence)
// No second queue/run model is introduced. All reads go through the existing
// db/ repositories. The console is functional-first; KPI analytics are out of
// scope (ENG-16).
// ---------------------------------------------------------------------------

import {
  listAgentWorkForStory,
  listAgentWorkItems,
  type AgentWorkItem,
} from '../db/agent-work'
import {
  getStoryboardStory,
  listStoryboardStories,
  listStoryRuns,
  type StoryboardStory,
  type StoryRun,
} from '../db/storyboard'

export type ConsoleStory = StoryboardStory & {
  latestRun: StoryRun | null
}

export type ConsoleSnapshot = {
  ready: boolean
  engineHealthy: boolean
  workerState: 'idle' | 'busy' | 'paused' | 'stale'
  activeCommandCount: number
  queuedReadyCount: number
  staleCount: number
  scheduler: 'unattended' | 'manual' | 'unknown'
  stories: ConsoleStory[]
  commands: AgentWorkItem[]
  runCount: number
}

export type ConsoleHealth = {
  ready: boolean
  engineHealthy: boolean
  workerState: ConsoleSnapshot['workerState']
  activeCommandCount: number
  queuedReadyCount: number
  staleCount: number
}

/**
 * Load the full console snapshot from the production/DEV control plane.
 * Returns `ready:false` when the storyboard tables are absent (migrations not
 * yet applied) so the UI can render a setup notice instead of crashing.
 */
export async function getCommandConsoleSnapshot(): Promise<ConsoleSnapshot> {
  const stories = await listStoryboardStories()
  if (!stories) {
    return {
      ready: false,
      engineHealthy: false,
      workerState: 'idle',
      activeCommandCount: 0,
      queuedReadyCount: 0,
      staleCount: 0,
      scheduler: 'unknown',
      stories: [],
      commands: [],
      runCount: 0,
    }
  }

  const commands = (await listAgentWorkItems()) ?? []
  const activeCommands = commands.filter((c) =>
    c.state === 'Claimed' || c.state === 'Running' || c.state === 'Paused',
  )
  const queuedReady = commands.filter((c) => c.state === 'Ready')

  const latestRunByStory = new Map<string, StoryRun>()
  for (const story of stories) {
    const runs = await listStoryRuns(story.id)
    if (runs.length > 0) latestRunByStory.set(story.id, runs[0])
  }

  const workerState: ConsoleSnapshot['workerState'] = activeCommands.some(
    (c) => c.state === 'Running',
  )
    ? 'busy'
    : activeCommands.some((c) => c.state === 'Paused')
      ? 'paused'
      : 'idle'

  return {
    ready: true,
    engineHealthy: true,
    workerState,
    activeCommandCount: activeCommands.length,
    queuedReadyCount: queuedReady.length,
    staleCount: 0,
    scheduler: 'unattended',
    stories: stories.map((s) => ({
      ...s,
      latestRun: latestRunByStory.get(s.id) ?? null,
    })),
    commands,
    runCount: latestRunByStory.size,
  }
}

// ---------------------------------------------------------------------------
// Story Execution Cockpit read model (ENG-20) — child/detail projection over
// the REAL control-plane tables only:
//   storyboard_story       (authoritative story specification)
//   agent_work_item        (durable command/queue for this story)
//   storyboard_story_run   (execution evidence for this story)
// No second queue/run model. This is a read projection — the lifecycle is
// never derived here into a new state machine.
// ---------------------------------------------------------------------------

export type StoryExecutionCockpit = {
  ready: boolean
  story: StoryboardStory | null
  workItems: AgentWorkItem[]
  runs: StoryRun[]
}

/**
 * Load the per-story execution cockpit. `ready:false` when the story does not
 * exist (unknown id / table absent) so the page can render a clean not-found.
 */
export async function getStoryExecutionCockpit(
  storyId: string,
): Promise<StoryExecutionCockpit> {
  const story = await getStoryboardStory(storyId)
  if (!story) {
    return { ready: false, story: null, workItems: [], runs: [] }
  }
  const [workItems, runs] = await Promise.all([
    listAgentWorkForStory(storyId),
    listStoryRuns(storyId),
  ])
  return {
    ready: true,
    story,
    workItems: workItems ?? [],
    runs,
  }
}

import {
  followFinishedLane,
  hydrateBareReadyItems,
} from '../agent-runtime/orchestrate-apply'
import {
  enqueueAgentWorkCommand,
  listAgentWorkItems,
} from '../db/agent-work'
import { getStoryboardStory } from '../db/storyboard'

export async function runForgeHydrate(): Promise<string[]> {
  return hydrateBareReadyItems({
    listItems: listAgentWorkItems,
    getStory: getStoryboardStory,
    enqueue: enqueueAgentWorkCommand,
  })
}

export async function runForgeFollow(input: {
  storyId: string
  finishedRole: string | null
  resultStatus?: string | null
}): Promise<string | null> {
  if (!input.finishedRole) return null
  return followFinishedLane({
    storyId: input.storyId,
    finishedRole: input.finishedRole,
    resultStatus: input.resultStatus,
    getStory: getStoryboardStory,
    enqueue: enqueueAgentWorkCommand,
  })
}

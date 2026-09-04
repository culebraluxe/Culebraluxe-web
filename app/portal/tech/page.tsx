import { redirect } from "next/navigation"

import { EngineeringCockpit } from "@/components/portal/tech/engineering-cockpit"
import { GatewayControl } from "@/components/portal/tech/gateway-control"
import { StoryBoardNotReady } from "@/components/portal/story-board"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { DEFAULT_FORGE_TEAM, listForgeTeamAssignments } from "@/agent-runtime/team"
import {
  buildStoryBoardCockpit,
  buildStoryBoardModel,
} from "@/lib/storyboard-data"
import {
  listActiveWork,
  listStoryExecutionSummaries,
  listStoryRuns,
  listStoryboardStories,
} from "@/db/storyboard"

export const dynamic = "force-dynamic"

export default async function TechPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>
}) {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const params = await searchParams
  const selectedId = typeof params.story === "string" ? params.story : null

  const [stories, executions] = await Promise.all([
    listStoryboardStories(),
    listStoryExecutionSummaries(),
  ])
  if (!stories) return <StoryBoardNotReady />

  const execMap = new Map(executions.map((e) => [e.storyId, e]))
  const withExecution = stories.map((s) => ({
    ...s,
    execution: execMap.get(s.id) ?? null,
  }))

  const model = buildStoryBoardModel(withExecution)
  const cockpit = buildStoryBoardCockpit(model)
  const activeQueue = await listActiveWork()

  const validId =
    selectedId && withExecution.some((s) => s.id === selectedId) ? selectedId : null
  const selectedStory = validId
    ? (withExecution.find((s) => s.id === validId) ?? null)
    : null
  const selectedIsActive = validId
    ? activeQueue.some((s) => s.id === validId)
    : false
  const runs = validId ? await listStoryRuns(validId) : []
  const freshness =
    withExecution.reduce((m, s) => (s.updatedAt > m ? s.updatedAt : m), "") ||
    new Date().toISOString()

  const routes = listForgeTeamAssignments().map((assignment) => ({
    position: assignment.position,
    profile: assignment.profile,
    player: assignment.player.name,
    harness: assignment.harness.name,
    field: assignment.field.name,
    provider: assignment.player.provider,
  }))

  return (
    <>
      <GatewayControl teamName={DEFAULT_FORGE_TEAM.name} routes={routes} />
      <EngineeringCockpit
        cockpit={cockpit}
        activeQueue={activeQueue}
        selectedStory={selectedStory}
        selectedIsActive={selectedIsActive}
        runs={runs}
        freshness={freshness}
      />
    </>
  )
}

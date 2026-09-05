import { createAgentRuntimeForgeRoleRunner } from '../workflow_app/forge/agent-runtime-role-runner'
import { driveForgeStory } from '../workflow_app/forge/forge-executor'

const args = process.argv.slice(2)
const value = (flag: string): string | undefined => {
  const index = args.indexOf(flag)
  return index >= 0 ? args[index + 1] : undefined
}

async function main(): Promise<void> {
  const storyId = value('--story')
  const workType = value('--work-type') ?? 'FEATURE'
  if (!storyId) {
    throw new Error(
      'usage: forge-engine-worker --story <story-id> [--work-type FEATURE|BUG|HOTFIX|RESEARCH|MIGRATION]',
    )
  }
  if (!['FEATURE', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION'].includes(workType)) {
    throw new Error(`invalid --work-type ${JSON.stringify(workType)}`)
  }
  const workerId = process.env.AGENT_WORKER_ID?.trim() || `forge-engine-${process.pid}`
  const result = await driveForgeStory(storyId, {
    start: { workType: workType as 'FEATURE' | 'BUG' | 'HOTFIX' | 'RESEARCH' | 'MIGRATION' },
    runner: createAgentRuntimeForgeRoleRunner({
      workerId,
      executionEnvironment: process.env.EXECUTION_ENV ?? 'DEV',
    }),
    workerId,
  })
  console.log(JSON.stringify(result, null, 2))
  if (result.exhausted) process.exitCode = 2
}

main().catch((error) => {
  console.error(String((error as Error)?.stack ?? error))
  process.exitCode = 1
})

import type { AgentCapability } from '../capabilities'

export type ForgeExecutionProvider = 'deepseek' | 'warp' | 'openclaw'

export type ProviderCommand = {
  bin: string
  args: string[]
  env?: Record<string, string | undefined>
}

export type ProviderCommandContext = {
  cwd: string
  task: string
  modelProfile: string
}

export interface ForgeProviderDescriptor {
  id: ForgeExecutionProvider
  description: string
  capabilities: AgentCapability[]
  buildCommand: (context: ProviderCommandContext) => ProviderCommand
}

function parseProvider(value: string | undefined, source: string): ForgeExecutionProvider {
  const normalized = (value ?? 'deepseek').trim().toLowerCase()
  if (normalized === 'deepseek' || normalized === 'warp' || normalized === 'openclaw') {
    return normalized
  }
  throw new Error(`unknown ${source} '${value}'`)
}

export function resolveForgeExecutionProvider(
  value = process.env.FORGE_EXECUTION_PROVIDER,
): ForgeExecutionProvider {
  return parseProvider(value, 'FORGE_EXECUTION_PROVIDER')
}

export function resolveForgeExecutionProviderForProfile(
  profile: string,
  env: NodeJS.ProcessEnv = process.env,
): ForgeExecutionProvider {
  const profileKey = `FORGE_PROVIDER_${profile.replace(/[^a-zA-Z0-9]+/g, '_').toUpperCase()}`
  const laneValue = env[profileKey]
  if (laneValue?.trim()) return parseProvider(laneValue, profileKey)
  return parseProvider(env.FORGE_EXECUTION_PROVIDER, 'FORGE_EXECUTION_PROVIDER')
}

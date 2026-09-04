import type { AgentCapability } from '../capabilities'

export type ForgeExecutionProvider = 'deepseek' | 'warp' | 'openclaw' | 'opencode'

export type ProviderCommand = {
  bin: string
  args: string[]
  env?: Record<string, string | undefined>
}

export type ProviderCommandContext = {
  cwd: string
  task: string
  modelProfile: string
  /** V6.1 runtime supplies these from the frozen work-item assignment. */
  playerId?: string
  providerId?: string
  modelId?: string
  /** Canonical provider/model reference for gateways that support explicit selection. */
  modelRef?: string
}

export interface ForgeProviderDescriptor {
  id: ForgeExecutionProvider
  description: string
  capabilities: AgentCapability[]
  buildCommand: (context: ProviderCommandContext) => ProviderCommand
}

function parseProvider(value: string | undefined, source: string): ForgeExecutionProvider {
  const normalized = (value ?? 'deepseek').trim().toLowerCase()
  if (
    normalized === 'deepseek' ||
    normalized === 'warp' ||
    normalized === 'openclaw' ||
    normalized === 'opencode'
  ) {
    return normalized
  }
  throw new Error(`unknown ${source} '${value}'`)
}

export function resolveForgeExecutionProvider(
  value = process.env.FORGE_EXECUTION_PROVIDER,
): ForgeExecutionProvider {
  return parseProvider(value, 'FORGE_EXECUTION_PROVIDER')
}

/** Legacy explicit override helper for gateway experiments only. */
export function resolveForgeExecutionProviderForProfile(
  profile: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): ForgeExecutionProvider {
  const profileKey = `FORGE_PROVIDER_${profile.replace(/[^a-zA-Z0-9]+/g, '_').toUpperCase()}`
  const profileValue = env[profileKey]
  if (profileValue?.trim()) return parseProvider(profileValue, profileKey)
  const globalValue = env.FORGE_EXECUTION_PROVIDER
  if (globalValue?.trim()) return parseProvider(globalValue, 'FORGE_EXECUTION_PROVIDER')
  return 'deepseek'
}

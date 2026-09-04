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

/**
 * Legacy explicit override helper for gateway experiments.
 *
 * IMPORTANT: this is NOT Forge's default role/model routing. V6 owns all
 * default position -> player -> harness selection in team.ts. With no explicit
 * environment override this returns the generic historical gateway default
 * only for backwards compatibility; core lane/factory code does not call it.
 */
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

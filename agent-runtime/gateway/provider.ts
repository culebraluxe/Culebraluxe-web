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

export function resolveForgeExecutionProvider(
  value = process.env.FORGE_EXECUTION_PROVIDER,
): ForgeExecutionProvider {
  const normalized = (value ?? 'deepseek').trim().toLowerCase()
  if (normalized === 'deepseek' || normalized === 'warp' || normalized === 'openclaw') {
    return normalized
  }
  throw new Error(`unknown FORGE_EXECUTION_PROVIDER '${value}'`)
}

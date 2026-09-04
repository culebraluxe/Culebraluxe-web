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

/**
 * ENG-FORGE-V5-03 — the DEFAULT execution provider for a logical profile when
 * NO explicit override (FORGE_PROVIDER_<PROFILE> or FORGE_EXECUTION_PROVIDER)
 * is present. The Smith `builder-flash` lane defaults to provider `opencode`
 * (adapter `opencode-harness`); every other Forge profile keeps the
 * forge-native DeepSeek harness default so scout/architect/verifier (Assay)
 * routing is unchanged.
 */
const DEFAULT_PROVIDER_FOR_PROFILE: Readonly<Record<string, ForgeExecutionProvider>> = {
  'builder-flash': 'opencode',
}

export function resolveForgeExecutionProvider(
  value = process.env.FORGE_EXECUTION_PROVIDER,
): ForgeExecutionProvider {
  return parseProvider(value, 'FORGE_EXECUTION_PROVIDER')
}

export function resolveForgeExecutionProviderForProfile(
  profile: string,
  // Index-signature view of the environment: accepts process.env as well as
  // deterministic literal envs in tests (Next augments NodeJS.ProcessEnv with
  // a REQUIRED NODE_ENV, which would reject plain test literals).
  env: Readonly<Record<string, string | undefined>> = process.env,
): ForgeExecutionProvider {
  // 1. An explicit per-profile override always wins (FORGE_PROVIDER_<PROFILE>).
  const profileKey = `FORGE_PROVIDER_${profile.replace(/[^a-zA-Z0-9]+/g, '_').toUpperCase()}`
  const laneValue = env[profileKey]
  if (laneValue?.trim()) return parseProvider(laneValue, profileKey)
  // 2. An explicit global override wins for every profile. A blank value is
  // treated as "no override" exactly like the per-profile position above.
  const globalValue = env.FORGE_EXECUTION_PROVIDER
  if (globalValue?.trim()) return parseProvider(globalValue, 'FORGE_EXECUTION_PROVIDER')
  // 3. No override -> per-profile default (ENG-FORGE-V5-03: builder-flash
  //    resolves to opencode; all other profiles stay on deepseek).
  return parseProvider(
    DEFAULT_PROVIDER_FOR_PROFILE[profile] ?? 'deepseek',
    `default for profile '${profile}'`,
  )
}

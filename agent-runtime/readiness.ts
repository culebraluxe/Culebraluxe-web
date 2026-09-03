import { accessSync, constants, existsSync } from 'node:fs'
import { delimiter, isAbsolute, join } from 'node:path'

export type AuthenticationReadiness = 'authenticated' | 'delegated' | 'required' | 'unknown'

export interface AdapterReadiness {
  registered: boolean
  installed: boolean
  authentication: AuthenticationReadiness
  ready: boolean
  reason: string
}

export function readyAdapterReadiness(
  authentication: AuthenticationReadiness = 'authenticated',
  reason = 'adapter is ready',
): AdapterReadiness {
  return {
    registered: true,
    installed: true,
    authentication,
    ready: true,
    reason,
  }
}

export function blockedAdapterReadiness(input: {
  installed: boolean
  authentication?: AuthenticationReadiness
  reason: string
}): AdapterReadiness {
  return {
    registered: true,
    installed: input.installed,
    authentication: input.authentication ?? 'unknown',
    ready: false,
    reason: input.reason,
  }
}

/**
 * Resolve whether a CLI entrypoint is executable on this host. Absolute and
 * slash-containing paths are checked directly; bare command names are resolved
 * against PATH. Injectable env keeps the probe deterministic in tests.
 */
export function commandIsInstalled(
  bin: string | null | undefined,
  env: NodeJS.ProcessEnv = process.env,
): boolean {
  const candidate = (bin ?? '').trim()
  if (!candidate) return false

  const executable = (path: string): boolean => {
    try {
      if (!existsSync(path)) return false
      accessSync(path, constants.X_OK)
      return true
    } catch {
      return false
    }
  }

  if (isAbsolute(candidate) || candidate.includes('/')) return executable(candidate)

  const pathValue = env.PATH ?? ''
  for (const dir of pathValue.split(delimiter)) {
    if (!dir) continue
    if (executable(join(dir, candidate))) return true
  }
  return false
}

export function explicitAuthenticationReady(
  value: string | undefined,
): AuthenticationReadiness {
  const normalized = (value ?? '').trim().toLowerCase()
  if (normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'authenticated') {
    return 'authenticated'
  }
  return 'required'
}

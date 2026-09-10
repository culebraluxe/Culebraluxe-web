// ---------------------------------------------------------------------------
// forge-story-reset-config — pure argv/env resolution for forge-story-reset.
//
// Side-effect free by contract (ENG-FORGE-V14): no process.exit, no Pool/DB,
// no process reads at module top level. Resolves the positional invocation
//   <story-id> [reset|recover] [dev|prod]
// plus the PROD safety gate: a prod target is REFUSED unless an explicit
// --force flag appears anywhere in argv. A refusal is reported as ok=false +
// error; the CLI wrapper (forge-story-reset.ts) exits 2 on it. When ok=true the
// mode/target fields are guaranteed members of their unions and url is set.
// ---------------------------------------------------------------------------

export type ForgeStoryResetMode = 'reset' | 'recover'
export type ForgeStoryResetTarget = 'dev' | 'prod'

export type ForgeStoryResetConfig =
  | {
      ok: true
      story: string
      mode: ForgeStoryResetMode
      target: ForgeStoryResetTarget
      force: boolean
      url: string
    }
  | { ok: false; error: string }

export const USAGE =
  'usage: forge-story-reset <story-id> [reset|recover] [dev|prod] [--force] (prod requires --force)'

/** Resolve one forge-story-reset invocation without touching the process. */
export function resolveStoryResetConfig(
  argv: string[],
  env: NodeJS.ProcessEnv,
): ForgeStoryResetConfig {
  const args = argv.slice(2)
  // --force is recognized position-independently and never occupies a
  // positional slot, so it may appear before, between, or after the positionals.
  const force = args.includes('--force')
  const positional = args.filter((arg) => arg !== '--force')

  const story = positional[0] ?? ''
  if (!story) return { ok: false, error: USAGE }

  const rawMode = (positional[1] ?? 'reset').toLowerCase()
  if (rawMode !== 'reset' && rawMode !== 'recover') {
    return {
      ok: false,
      error: `unknown mode ${JSON.stringify(rawMode)} (expected reset|recover)`,
    }
  }

  const rawTarget = (
    positional[2] ?? (env.APP_ENV === 'production' ? 'prod' : 'dev')
  ).toLowerCase()
  if (rawTarget !== 'prod' && rawTarget !== 'dev') {
    return {
      ok: false,
      error: `unknown target ${JSON.stringify(rawTarget)}`,
    }
  }

  if (rawTarget === 'prod' && !force) {
    return {
      ok: false,
      error:
        'prod target requires --force (refusing destructive reset/recover without explicit confirmation)',
    }
  }

  const url = rawTarget === 'prod' ? env.DATABASE_URL_PROD : env.DATABASE_URL_DEV
  if (!url) {
    return {
      ok: false,
      error: `no ${rawTarget.toUpperCase()} DATABASE_URL configured`,
    }
  }

  return { ok: true, story, mode: rawMode, target: rawTarget, force, url }
}

// ---------------------------------------------------------------------------
// forge-story-reset-config — pure argv/env resolution for forge-story-reset.
//
// Side-effect free by contract (ENG-FORGE-V14): no process.exit, no Pool/DB, no
// process reads at module top level. Resolves:
//
//   <story-id> [reset|recover] [--force]
//
// THE TARGET IS NOT A CHOICE. It used to be a positional (`[dev|prod]`) defaulted
// from APP_ENV, which made this operator script its own arbiter of which database
// it was about to destructively modify. Captain, 2026-09-12: "always PROD but this
// should be domain responsibility of the Pool Manager — it should not even have the
// ability to make that choice." The environment now comes from the ONE declaration
// (lib/execution-target), and this tool refuses to run anywhere that is not PROD.
//
// `--force` stays, because that gate is about confirming a DESTRUCTIVE act, not
// about picking an environment. A refusal is ok=false + reason; the CLI exits 2.
// ---------------------------------------------------------------------------

import { describeControlPlane } from '../lib/execution-target'
import { forgeDbConnectionString } from '../db/forge-db'

export type ForgeStoryResetMode = 'reset' | 'recover'
export type ForgeStoryResetTarget = 'prod' | 'dev'

export type ForgeStoryResetConfig =
  | {
      ok: true
      story: string
      mode: ForgeStoryResetMode
      /** Always 'prod': the pool manager decides, and this tool refuses anything else. */
      target: ForgeStoryResetTarget
      force: boolean
    }
  | { ok: false; error: string }

export const USAGE =
  'usage: forge-story-reset <story-id> [reset|recover] [--force] ' +
  '(the database target is PROD and is decided by the pool manager; PROD requires --force)'

/** Resolve one forge-story-reset invocation without touching the process. */
export function resolveStoryResetConfig(
  argv: string[],
  env: NodeJS.ProcessEnv,
): ForgeStoryResetConfig {
  const args = argv.slice(2)
  // --force is recognized position-independently and never occupies a positional
  // slot, so it may appear before, between, or after the positionals.
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

  // Any third positional is an attempt to choose the environment. Say so, and say
  // who owns that decision, rather than silently ignoring it.
  if (positional[2] !== undefined) {
    return {
      ok: false,
      error:
        `unexpected argument ${JSON.stringify(positional[2])}: the database target is not a choice ` +
        'here — the pool manager declares the environment. ' +
        USAGE,
    }
  }

  const declared = describeControlPlane(env)
  if (!declared.target) {
    return { ok: false, error: declared.reason ?? 'the database environment is not declared' }
  }
  if (declared.target !== 'prod') {
    return {
      ok: false,
      error:
        `refusing to run against ${declared.target.toUpperCase()}: this tool only resets PROD state, ` +
        "and the environment is the pool manager's decision, not the caller's. " +
        'Declare APP_ENV=production (or run under Vercel).',
    }
  }

  if (!force) {
    return {
      ok: false,
      error:
        'PROD reset/recover requires --force (refusing a destructive act without explicit confirmation)',
    }
  }

  try {
    forgeDbConnectionString('prod', env)
  } catch (error) {
    return { ok: false, error: (error as Error).message }
  }

  return { ok: true, story, mode: rawMode, target: 'prod', force }
}

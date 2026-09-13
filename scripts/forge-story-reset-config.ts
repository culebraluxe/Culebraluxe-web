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

export type ForgeStoryResetMode = 'reset' | 'recover' | 'clean'
export type ForgeStoryResetTarget = 'prod' | 'dev'

export type ForgeStoryResetConfig =
  | {
      ok: true
      /** Empty for `clean`, which sweeps every story: hygiene is not story-scoped. */
      story: string
      mode: ForgeStoryResetMode
      /** Always 'prod': the pool manager decides, and this tool refuses anything else. */
      target: ForgeStoryResetTarget
      force: boolean
      /** `clean` only: how old a claim must be before it counts as abandoned (default 15). */
      staleMinutes: number
    }
  | { ok: false; error: string }

/** A claim younger than this is treated as LIVE: `clean` must never cancel a running peer. */
export const DEFAULT_CLEAN_STALE_MINUTES = 15

export const USAGE =
  'usage: forge-story-reset <story-id> [reset|recover] [--force] | forge-story-reset clean ' +
  '[--stale-minutes N] [--force] ' +
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
  const staleMinutes = readStaleMinutes(args)
  if (staleMinutes === null) {
    return { ok: false, error: `--stale-minutes needs a positive number. ${USAGE}` }
  }
  const positional = stripValuedFlags(args, '--stale-minutes').filter((arg) => arg !== '--force')

  // `clean` may lead (hygiene reads naturally as its own verb) or follow a story id. When
  // it leads there is no story, because the sweep is about the CONTROL PLANE's leftovers,
  // not about one story's chain.
  const cleanLeads = positional[0]?.toLowerCase() === 'clean'
  const story = cleanLeads ? '' : (positional[0] ?? '')
  if (!cleanLeads && !story) return { ok: false, error: USAGE }

  const rawMode = (cleanLeads ? 'clean' : (positional[1] ?? 'reset')).toLowerCase()
  if (rawMode !== 'reset' && rawMode !== 'recover' && rawMode !== 'clean') {
    return {
      ok: false,
      error: `unknown mode ${JSON.stringify(rawMode)} (expected reset|recover|clean)`,
    }
  }

  // Any leftover positional is an attempt to choose the environment. Say so, and say
  // who owns that decision, rather than silently ignoring it.
  const consumed = cleanLeads ? 1 : 2
  if (positional[consumed] !== undefined) {
    return {
      ok: false,
      error:
        `unexpected argument ${JSON.stringify(positional[consumed])}: the database target is not a choice ` +
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
        'PROD reset/recover/clean requires --force (refusing a destructive act without explicit confirmation)',
    }
  }

  try {
    forgeDbConnectionString('prod', env)
  } catch (error) {
    return { ok: false, error: (error as Error).message }
  }

  return { ok: true, story, mode: rawMode, target: 'prod', force, staleMinutes }
}

/** Read `--stale-minutes N`; null when present but not a positive number. */
function readStaleMinutes(args: string[]): number | null {
  const i = args.indexOf('--stale-minutes')
  if (i < 0) return DEFAULT_CLEAN_STALE_MINUTES
  const raw = args[i + 1]
  if (raw === undefined) return null
  const n = Number(raw)
  return Number.isFinite(n) && n > 0 ? n : null
}

/** Drop a flag together with its value, so the value is never mistaken for a positional. */
function stripValuedFlags(args: string[], flag: string): string[] {
  const out: string[] = []
  for (let i = 0; i < args.length; i++) {
    if (args[i] === flag) {
      i++
      continue
    }
    out.push(args[i])
  }
  return out
}

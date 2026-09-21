#!/usr/bin/env node
import {
  FORGE_HOLD_RESUME_TARGETS,
  resolveForgeHold,
  validResumeTarget,
  type ForgeHoldResolution,
} from '@/legacy/workflow_app/forge/forge-hold-resolve'

// ---------------------------------------------------------------------------
// ENG-FORGE-RESUME-DOOR-01 — ONE OPERATOR DOOR OUT OF A HELD OR FAILED RUN.
//
// A run sitting on a hold OR stopped at a failed lane can be resumed at a named
// node or cancelled from here. The durable forge_hold_record row carries who,
// when and why; a run that cannot be moved prints the missing piece and exits
// non-zero instead of failing silently.
//
//   node --import tsx --env-file=.env.local scripts/forge-resume-door.ts \
//     --story <STORY-ID> --by <operator> (--to <NODE> | --cancel | --fail) \
//     [--reason "<why it moved>"]
// ---------------------------------------------------------------------------

const args = process.argv.slice(2)
const arg = (name: string): string | null => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 ? args[i + 1] ?? null : null
}
const has = (name: string): boolean => args.includes(`--${name}`)

function usage(): never {
  console.error(
    'forge-resume-door: need --story <STORY-ID> --by <operator> and exactly one of ' +
      '--to <NODE>, --cancel, --fail. Accepted NODE values: ' +
      [...FORGE_HOLD_RESUME_TARGETS].join(', ') +
      ' (--to CANCEL is the same as --cancel).',
  )
  process.exit(2)
}

async function main(): Promise<void> {
  const storyId = (arg('story') ?? '').trim()
  const resolver = (arg('by') ?? '').trim()
  const to = (arg('to') ?? '').trim().toUpperCase()
  const cancel = has('cancel') || to === 'CANCEL'
  const fail = has('fail')
  const reason = arg('reason') ?? undefined
  const note = arg('note') ?? undefined

  if (!storyId || !resolver) usage()

  const moves = [to && !cancel ? 'resolve' : null, cancel ? 'cancel' : null, fail ? 'fail' : null].filter(
    Boolean,
  )
  if (moves.length !== 1) {
    console.error('forge-resume-door: state exactly one of --to <NODE>, --cancel, --fail')
    process.exit(2)
  }

  let input: ForgeHoldResolution & { storyId: string }
  if (to && !cancel) {
    if (!validResumeTarget(to)) {
      console.error(
        `forge-resume-door: '${to}' is not a resume target. Accepted: ` +
          [...FORGE_HOLD_RESUME_TARGETS].join(', '),
      )
      process.exit(2)
    }
    input = { resolution: 'resolve', resumeTarget: to, storyId, resolver, reason, note }
  } else if (cancel) {
    input = { resolution: 'cancel', storyId, resolver, reason, note }
  } else {
    input = { resolution: 'fail', storyId, resolver, reason, note }
  }

  const at = new Date().toISOString()
  const result = await resolveForgeHold(input)

  if (result.outcome === 'refused') {
    console.error(
      `forge-resume-door: CANNOT MOVE ${storyId} — ${result.missing} ` +
        `(asked by ${resolver} at ${at})`,
    )
    process.exit(1)
  }

  const why = reason ?? note ?? 'Forge HOLD'
  console.log(
    `forge-resume-door: ${storyId} ` +
      `${result.movedTo ? `resumed at ${result.movedTo}` : 'terminated'} ` +
      `from node '${result.stopNode}' by ${resolver} at ${at} — ${why} ` +
      `(forge_hold_record ${result.auditId})`,
  )
}

main().catch((error) => {
  console.error(`forge-resume-door: ${(error as Error)?.message ?? String(error)}`)
  process.exit(1)
})

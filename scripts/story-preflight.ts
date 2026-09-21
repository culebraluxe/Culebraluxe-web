#!/usr/bin/env node
// ---------------------------------------------------------------------------
// story-preflight — DO THE DOORS HAVE THEIR KEYS BEFORE A LANE RUNS?
//
// WHY THIS EXISTS (2026-09-18): three engine holds in one evening, and every one of them was an
// AUTHORING gap rather than an engine limit —
//   · the Architect held BATCH-RECEIPT-01: "allowed seams: <none declared>"
//   · the Lead held MIGRATION-LINT-01: "acceptance clauses 1, 2 and 4 have no assertion in the
//     frozen proof, so QA must report them UNPROVEN"
//   · the Assay then refused the SAME story: "the frozen story proof is a malformed compound shell
//     string" (prose mixed into the command).
// A lane needs the same three things a solo run would simply HAVE: a runnable proof, declared
// seams, and an acceptance-to-assertion map. This reports them per story so a dispatch is not a
// lottery ticket, and exits 1 when a dispatchable story is missing a key.
//
// Usage: pnpm story:preflight [--batch N] [--story ID] [--all]
//   Default: dispatchable stories only (Planned / Ready / In Progress).
// ---------------------------------------------------------------------------
import { sql } from '@/legacy/db/client'

const argv = process.argv.slice(2)
const val = (flag: string): string | null => {
  const i = argv.indexOf(flag)
  return i >= 0 ? (argv[i + 1] ?? null) : null
}
const batch = val('--batch')
const story = val('--story')

type Key = 'proof' | 'seams' | 'lockfile'
type Row = {
  id: string
  batch: number | null
  status: string
  proof: string | null
  scope: string | null
  assertions: Record<string, unknown> | null
}

async function main(): Promise<void> {
  const rows = (await sql`
    select id, batch, status, assay_commands as proof, scope, acceptance_assertions as assertions
    from storyboard_story
    where status in ('Planned', 'Ready', 'In Progress')
      and (${batch}::int is null or batch = ${batch}::int)
      and (${story}::text is null or id = ${story}::text)
    order by batch nulls last, id
  `) as unknown as Row[]

/** A proof is a LIST OF COMMANDS: backticked commands or bare paths, one per line, no prose. */
function proofKey(proof: string | null): { ok: boolean; why: string } {
  if (!proof || !proof.trim()) return { ok: false, why: 'no proof declared' }
  const lines = proof.split('\n').map((l) => l.trim()).filter(Boolean)
  if (lines.length === 0) return { ok: false, why: 'proof is empty' }
  const bad: string[] = []
  for (const line of lines) {
    if (!line.startsWith('-')) {
      bad.push(`line is not a list item: ${line.slice(0, 40)}`)
      continue
    }
    if (!line.includes('`')) {
      bad.push(`line has no runnable command: ${line.slice(0, 40)}`)
      continue
    }
    // Prose and operators OUTSIDE the backticks are what made the Assay refuse: "(new fence: …) +".
    const outside = line.replace(/`[^`]*`/g, '')
    if (/[()[\]]|\+/.test(outside)) {
      bad.push(`prose outside the command: ${line.slice(0, 40)}`)
    }
  }
  return bad.length === 0
    ? { ok: true, why: `${lines.length} command(s)` }
    : { ok: false, why: bad[0] }
}

/** The Architect needs repository-relative paths to declare as seams. A story that names files OR
 *  directories is preflighted; a story that only describes behaviour cannot be routed.
 *
 *  DIRECTORIES COUNT, and that is measured rather than assumed: the Architect's own refusal on
 *  BATCH-RECEIPT-01 listed `legacy/workflow_app/tests` (a directory) among the paths it wanted declared as
 *  seams. An earlier version of this check required a file extension and flagged a story whose scope
 *  named `legacy/workflow_app/forge/agents/qa/` — a false gap, which is worse than no check because it
 *  teaches the reader to ignore the tool. */
function seamsKey(scope: string | null): { ok: boolean; why: string } {
  if (!scope || !scope.trim()) return { ok: false, why: 'no scope declared' }
  const paths = scope.match(/[A-Za-z0-9_.-]+\/[A-Za-z0-9_./-]*/g) ?? []
  return paths.length > 0
    ? { ok: true, why: `${paths.length} path(s)` }
    : { ok: false, why: 'scope names no repository-relative path' }
}

/** Every acceptance clause must carry at least one assertion reference, or QA must rule UNPROVEN.
 *
 *  THIS IS A WARNING, NOT A GAP, and the distinction is the whole point of the supplier: the
 *  acceptance map has TWO declaration places, and the runner resolves them in a stated order —
 *  the HANDOFF declaration (the Lead's contract row under lead_pre, else the Architect's contract
 *  in the brief) WINS, and the STORY-AUTHOR row is the FALLBACK (ENG-FORGE-ACCEPTANCE-SUPPLIER-01).
 *  So a story-row mapping that is absent is not fatal: it means the LANE must declare one, and QA
 *  rules `UNPROVEN acceptance-map-missing` if it does not. Pre-supplying it here is still the
 *  stronger setup for a story whose fence already exists, because it removes that dependency. */
function assertionsKey(a: Record<string, unknown> | null): { ok: boolean; why: string } {
  if (!a || typeof a !== 'object') {
    return { ok: false, why: 'story-row fallback absent — the Lead/Architect contract must declare it' }
  }
  const entries = Object.entries(a)
  if (entries.length === 0) {
    return { ok: false, why: 'story-row fallback is empty — the lane must declare the map' }
  }
  const uncovered = entries.filter(([, v]) => !Array.isArray(v) || v.length === 0)
  return uncovered.length === 0
    ? { ok: true, why: `${entries.length} clause(s) covered` }
    : { ok: false, why: `${uncovered.length}/${entries.length} clause(s) have no assertion` }
}

/** A dependency-adding story must carry the lockfile in its declared scope.
 *
 *  MEASURED 2026-09-18: PROPERTY-INVARIANTS-01 added `fast-check` to package.json and installed it,
 *  but its published candidate did NOT include pnpm-lock.yaml — so `pnpm install --frozen-lockfile`
 *  failed in CI with ERR_PNPM_OUTDATED_LOCKFILE and main went red over a dependency the lane had
 *  every right to add. The engine's publish path commits the story's DECLARED surface, so a file
 *  outside that surface is left behind — and the lockfile is part of what a dependency change IS. */
function lockfileKey(input: {
  scope: string | null
  proof: string | null
}): { ok: boolean; why: string } {
  const text = `${input.scope ?? ''}\n${input.proof ?? ''}`
  const touchesDeps = /package\.json|fast-check|zod|valibot|arktype/.test(text)
  if (!touchesDeps) return { ok: true, why: 'no dependency change declared' }
  return /pnpm-lock\.yaml/.test(text)
    ? { ok: true, why: 'lockfile declared alongside the dependency' }
    : {
        ok: false,
        why: 'declares a dependency change but not pnpm-lock.yaml — the frozen install will fail',
      }
}

/** A HARD key: without it a door refuses before any work can be judged. */

  let missing = 0
  let warned = 0
  console.log('story preflight — do the doors have their keys?\n')
  for (const r of rows) {
    const hard: Record<Key, { ok: boolean; why: string }> = {
      proof: proofKey(r.proof),
      seams: seamsKey(r.scope),
      lockfile: lockfileKey({ scope: r.scope, proof: r.proof }),
    }
    const soft = assertionsKey(r.assertions)
    const gaps = (Object.keys(hard) as Key[]).filter((k) => !hard[k].ok)
    if (gaps.length > 0) missing += 1
    else if (!soft.ok) warned += 1
    const mark =
      gaps.length > 0 ? `GAP(${gaps.join(',')})` : soft.ok ? 'READY' : 'WARN(acceptance-map)'
    console.log(
      `${mark.padEnd(28)} b${String(r.batch ?? '-').padEnd(4)} ${r.status.padEnd(12)} ${r.id}`,
    )
    for (const g of gaps) console.log(`    ${g}: ${hard[g].why}`)
    if (gaps.length === 0 && !soft.ok) console.log(`    acceptance map: ${soft.why}`)
  }
  console.log(
    `\n${rows.length} story(ies) in scope · ${rows.length - missing - warned} ready · ` +
      `${warned} lane-dependent · ${missing} with a hard gap`,
  )
  if (missing > 0) {
    console.log(
      'A gap is not a blocker to think about later: each one becomes a hold at the door named above, ' +
        'after a lane and its tokens have already been spent.',
    )
    process.exit(1)
  }
  process.exit(0)
}

main().catch((error) => {
  console.error(String((error as Error)?.stack ?? error))
  process.exit(1)
})

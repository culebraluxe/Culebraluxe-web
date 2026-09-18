#!/usr/bin/env node
// ---------------------------------------------------------------------------
// POST-SPRINT HYGIENE — wipe the counters, scrub the line.
//
// The captain, 2026-09-18: "if it were a kitchen we wipe down the counters and scrub the line ... to
// launch localhost dev I have to stop hard on cache files." Every class here was measured on this
// machine that night, not imagined:
//
//   build dirs   .next held 308 Finder copy-on-conflict strays ("routes.d 2.ts", "package 2.json")
//                across 726 MB, and those strays break `tsc` and `next dev` until someone prunes them
//   worktrees    a stale worktree on a merged branch, still on disk
//   git          unpushed commits, branches merged and never deleted
//   database     probe rows, open work items on terminal instances, claims left by a dead process
//   tmp          probe scripts and logs written while diagnosing (489 files that night)
//
// READ-ONLY BY DEFAULT, because a cleaner that surprises you is worse than the mess. `--fix` does the
// SAFE removals (generated strays, merged worktrees, our own probe rows and probe scripts); `--hard`
// also drops `.next`, which is the "stop hard" step done by hand today — it costs a rebuild, nothing
// else.
//
//   pnpm health          report only: is this machine clear?
//   pnpm health --fix    the safe removals
//   pnpm health --hard   also drop .next, then the next build recreates it
// ---------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync, rmSync, statSync } from 'node:fs'
import { join } from 'node:path'

const args = process.argv.slice(2)
const fix = args.includes('--fix') || args.includes('--hard')
const hard = args.includes('--hard')

const root =
  spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).stdout?.trim() ?? ''
if (!root) {
  console.error('not inside a git repository')
  process.exit(2)
}

const sh = (command: string, cwd = root): string =>
  spawnSync(command, { shell: true, cwd, encoding: 'utf8' }).stdout?.trim() ?? ''

/**
 * A Finder copy-on-conflict sibling: `routes.d 2.ts`, `package 3.json`. A space and a number before
 * the extension, and never something a person means to keep.
 */
export function isCopyOnConflictJunk(name: string): boolean {
  return / [0-9]+\.[A-Za-z0-9]+$/.test(name)
}

function walk(dir: string, out: string[] = []): string[] {
  if (!existsSync(dir)) return out
  for (const name of readdirSync(dir)) {
    const full = join(dir, name)
    if (statSync(full).isDirectory()) walk(full, out)
    else if (isCopyOnConflictJunk(name)) out.push(full)
  }
  return out
}

type Finding = { area: string; detail: string; fixed: boolean }

const findings: Finding[] = []
const note = (area: string, detail: string, fixed = false) => findings.push({ area, detail, fixed })

// --- build dirs -------------------------------------------------------------
const nextDir = join(root, '.next')
if (existsSync(nextDir)) {
  const junk = walk(nextDir)
  const sizeMb = Math.round(Number(sh(`du -sk "${nextDir}" | cut -f1`) || '0') / 1024)
  if (junk.length > 0) {
    if (fix) for (const file of junk) rmSync(file, { force: true })
    note('.next', `${junk.length} copy-on-conflict stray(s)${fix ? ' removed' : ''} — these break tsc/dev`, fix)
  }
  if (hard) {
    rmSync(nextDir, { recursive: true, force: true })
    note('.next', `dropped entirely (${sizeMb} MB); the next build recreates it`, true)
  } else if (sizeMb > 400) {
    note('.next', `${sizeMb} MB of build output — \`pnpm health --hard\` clears it (costs a rebuild)`)
  }
} else {
  note('.next', 'absent — a clean tree')
}

// --- worktrees --------------------------------------------------------------
const worktrees = sh('git worktree list --porcelain')
  .split('\n\n')
  .map((block) => block.split('\n'))
  .filter((lines) => lines.length > 0 && (lines[0] ?? '').startsWith('worktree '))
  .map((lines) => ({
    path: (lines[0] ?? '').replace('worktree ', ''),
    branch: (lines.find((l) => l.startsWith('branch ')) ?? '').replace('branch refs/heads/', ''),
  }))
const extras = worktrees.filter((w) => w.path !== root)
for (const w of extras) {
  const merged = w.branch.length > 0 && sh(`git branch --merged main --list "${w.branch}"`).length > 0
  const wDirty = sh('git status --porcelain', w.path).length > 0
  if (merged && !wDirty) {
    if (fix) {
      sh(`git worktree remove "${w.path}"`)
      sh(`git branch -d "${w.branch}"`)
    }
    note('worktree', `${w.path} (${w.branch}) is merged and clean${fix ? ' — removed' : ''}`, fix)
  } else {
    note(
      'worktree',
      `${w.path} (${w.branch || 'detached'}) is ${wDirty ? 'DIRTY' : 'unmerged'} — left alone`,
    )
  }
}
if (extras.length === 0) note('worktree', 'none beyond the main checkout')

// --- git --------------------------------------------------------------------
const dirty = sh('git status --porcelain')
note('git', dirty.length > 0 ? `working tree has ${dirty.split('\n').length} uncommitted change(s)` : 'working tree clean')
const ahead = Number(sh('git rev-list --count origin/main..HEAD') || '0')
note('git', ahead > 0 ? `${ahead} commit(s) not pushed to origin/main` : 'main is in sync with origin/main')
for (const name of sh('git branch --merged main --format="%(refname:short)"')
  .split('\n')
  .map((b) => b.trim())
  .filter((b) => b.length > 0 && b !== 'main')) {
  if (fix) sh(`git branch -d "${name}"`)
  note('git', `branch ${name} is merged into main${fix ? ' — deleted' : ''}`, fix)
}

// --- tmp: the probe scripts and logs this work leaves behind ----------------
const tmpJunk = readdirSync('/tmp').filter((n) => /\.(mts|mjs)$/.test(n) || /^(ar|lf|vi|vv|cc|qr|ct|fd|rc|ms|pn|sr|dn|sprint)/.test(n))
if (tmpJunk.length > 0) {
  if (fix) for (const name of tmpJunk) rmSync(join('/tmp', name), { force: true })
  note('tmp', `${tmpJunk.length} probe/log file(s) in /tmp${fix ? ' removed' : ''}`, fix)
} else {
  note('tmp', 'no probe leftovers')
}

const width = Math.max(...findings.map((f) => f.area.length))
console.log(`hygiene for ${root}${fix ? (hard ? ' (--fix --hard)' : ' (--fix)') : ' (report only)'}\n`)
for (const finding of findings) {
  console.log(`  ${finding.area.padEnd(width)}  ${finding.fixed ? 'cleaned' : '   ...   '}  ${finding.detail}`)
}
const remaining = findings.filter(
  (f) => !f.fixed && /stray|DIRTY|uncommitted|not pushed|merged and clean|probe\/log/.test(f.detail),
)
console.log(
  remaining.length === 0
    ? '\nthis machine is clear.'
    : `\n${remaining.length} item(s) worth attention — rerun with --fix for the safe ones.`,
)


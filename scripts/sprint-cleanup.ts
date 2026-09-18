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
// SAFE removals (generated strays, merged worktrees, our own probe rows and probe scripts, and
// leftover branches whose work is PROVEN to be in main or which are test fixtures); `--hard` also
// drops `.next`, which is the "stop hard" step done by hand today — it costs a rebuild, nothing else.
// `--remote` extends the branch sweep to origin, and is off by default because it affects everyone
// who shares the repo.
//
//   pnpm health              report only: is this machine clear?
//   pnpm health --fix        the safe removals
//   pnpm health --fix --remote   also delete the proven-landed and test branches on origin
//   pnpm health --hard       also drop .next, then the next build recreates it
// ---------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'
import { appendFileSync, existsSync, lstatSync, readdirSync, rmSync } from 'node:fs'
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

// Conflict copies are the debris of a cloud-sync client (see lib/git/sync-conflict.ts — it is OneDrive
// known-folder-move on this machine, not iCloud) and the predicate lives there so it can be FENCED
// without executing this script.
import { conflictCopies, isConflictCopyName } from '../lib/git/sync-conflict'

// Branch judgement lives in lib/git/branch-hygiene.ts so it can be FENCED without executing this
// script (importing a script runs it). The rule is: land is judged by PATCH, not by ancestry, and
// only branches the tooling created are ever deleted.
import {
  MACHINE_OWNED_BRANCH,
  TEST_SCAFFOLDING_BRANCH,
  branchClassOf,
  branchIsDeletable,
} from '../lib/git/branch-hygiene'

function unappliedPatchCount(branch: string, base: string): number {
  return sh(`git cherry ${base} "${branch}" 2>/dev/null`)
    .split('\n')
    .filter((line) => line.startsWith('+')).length
}

// Every file under `dir`, so conflict copies can be judged against the ORIGINAL beside them (a name
// pattern alone would also catch `CHANGELOG 2026.md`).
//
// lstat, NEVER stat, and NEVER descend a symlink. This crashed on its first real run: `.pnpm-store`
// holds symlinks whose targets are gone (pnpm's own bookkeeping), `statSync` follows the link and throws
// ENOENT, and the whole counter-wipe died on a path it had no business walking. A walker that only
// classifies debris has no reason to follow a link anywhere.
function walkFiles(dir: string, out: string[] = [], skip: Set<string> = new Set()): string[] {
  if (!existsSync(dir)) return out
  for (const name of readdirSync(dir)) {
    if (skip.has(name)) continue
    const full = join(dir, name)
    const stats = lstatSync(full)
    if (stats.isSymbolicLink()) {
      if (isConflictCopyName(name)) out.push(full)
      continue
    }
    if (stats.isDirectory()) {
      if (isConflictCopyName(name)) out.push(full)
      else walkFiles(full, out, skip)
    } else out.push(full)
  }
  return out
}

type Finding = { area: string; detail: string; fixed: boolean }

const findings: Finding[] = []
const note = (area: string, detail: string, fixed = false) => findings.push({ area, detail, fixed })

// --- build dirs -------------------------------------------------------------
// RULE OF THUMB, so nobody has to ask again: `.next` is disposable and is never committed. What SHIPS
// is `server` + `static` (tens of MB); `cache` is the webpack/turbopack cache and is usually 90%+ of
// the directory. So a large `.next` is not a problem by itself — the signals are copy-on-conflict
// strays (they break `tsc` and `next dev`), and a cache many times the size of the shipped artifacts,
// where `--hard` costs one cold build and nothing else. As a ceiling: `.next` should stay smaller than
// `node_modules`; this app measured 726 MB against 1.4 GB, i.e. healthy.
const nextDir = join(root, '.next')
if (existsSync(nextDir)) {
  const junk = conflictCopies(walkFiles(nextDir))
  const kb = (path: string) => Number(sh(`du -sk "${path}" 2>/dev/null | cut -f1`) || '0')
  const sizeMb = Math.round(kb(nextDir) / 1024)
  const cacheMb = Math.round(kb(join(nextDir, 'cache')) / 1024)
  const shipsMb = Math.round((kb(join(nextDir, 'server')) + kb(join(nextDir, 'static'))) / 1024)
  if (junk.length > 0) {
    if (fix) for (const file of junk) rmSync(file, { force: true })
    note('.next', `${junk.length} copy-on-conflict stray(s)${fix ? ' removed' : ''} — these break tsc/dev`, fix)
  }
  if (hard) {
    rmSync(nextDir, { recursive: true, force: true })
    note('.next', `dropped entirely (${sizeMb} MB: ${cacheMb} MB cache, ${shipsMb} MB shipped); the next build recreates it`, true)
  } else if (cacheMb > 300 && cacheMb > shipsMb * 5) {
    note(
      '.next',
      `${sizeMb} MB, of which ${cacheMb} MB is cache and only ${shipsMb} MB ships — \`pnpm health --hard\` clears the cache for the price of one cold build`,
    )
  } else {
    note('.next', `${sizeMb} MB (${cacheMb} MB cache, ${shipsMb} MB shipped) — healthy, nothing to do`)
  }
} else {
  note('.next', 'absent — a clean tree, and the next build recreates it')
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
  // LANDEDNESS IS JUDGED BY PATCH HERE TOO. This used `git branch --merged main`, the same ancestry
  // test that failed for branches: a worktree whose branch landed by rebase looked "unmerged" forever
  // and was never removed. The branch is NOT deleted here — removing a worktree removes a directory,
  // and branches are swept above with their own proof and ownership guard (a branch still checked out
  // in a worktree is skipped there, so this loop must clear the worktree first for the sweep to see it).
  const wLanded = w.branch.length > 0 && unappliedPatchCount(w.branch, 'main') === 0
  const wFixture = w.branch.length > 0 && TEST_SCAFFOLDING_BRANCH.test(w.branch)
  const wDirty = sh('git status --porcelain', w.path).length > 0
  if ((wLanded || wFixture) && !wDirty) {
    if (fix) sh(`git worktree remove "${w.path}"`)
    note(
      'worktree',
      `${w.path} (${w.branch}) is ${wLanded ? 'landed' : 'a test fixture'} and clean${fix ? ' — removed (branch swept separately)' : ''}`,
      fix,
    )
  } else {
    note(
      'worktree',
      `${w.path} (${w.branch || 'detached'}) is ${wDirty ? 'DIRTY' : 'unlanded'} — left alone`,
    )
  }
}
if (extras.length === 0) note('worktree', 'none beyond the main checkout')

// --- git --------------------------------------------------------------------
const dirty = sh('git status --porcelain')
note('git', dirty.length > 0 ? `working tree has ${dirty.split('\n').length} uncommitted change(s)` : 'working tree clean')
const ahead = Number(sh('git rev-list --count origin/main..HEAD') || '0')
note('git', ahead > 0 ? `${ahead} commit(s) not pushed to origin/main` : 'main is in sync with origin/main')

// Leftover lane and test branches. Deletion is proven per branch, never assumed: a branch is
// removed only when every one of its patches is already in main, or it is a test fixture, and
// NEVER when a worktree has it checked out. Force delete is deliberate — `git branch -d` refuses
// rebase-landed work, and that refusal is exactly what let 168 branches pile up.
const worktreeBranches = new Set(
  sh('git worktree list --porcelain')
    .split('\n')
    .filter((line) => line.startsWith('branch refs/heads/'))
    .map((line) => line.replace('branch refs/heads/', '').trim()),
)
const remote = args.includes('--remote') || args.includes('--hard-remote')
const deletedTips: string[] = []
const tally: Record<string, number> = {}

for (const name of sh('git branch --format="%(refname:short)"')
  .split('\n')
  .map((b) => b.trim())
  .filter((b) => b.length > 0 && b !== 'main' && !worktreeBranches.has(b))) {
  const cls = branchClassOf({
    name,
    unappliedPatches: unappliedPatchCount(name, 'main'),
    inWorktree: false,
  })
  tally[cls] = (tally[cls] ?? 0) + 1
  if (!branchIsDeletable(cls)) continue
  if (!MACHINE_OWNED_BRANCH.test(name)) {
    note('git', `branch ${name} is landed but is NOT ours (a person named it) — left for you`)
    continue
  }
  const tip = sh(`git rev-parse "${name}"`)
  if (fix) {
    sh(`git branch -D "${name}"`)
    deletedTips.push(`${name} ${tip}`)
  }
  note(
    'git',
    `branch ${name} is ${cls === 'landed' ? 'fully landed in main' : 'test scaffolding'}${fix ? ` — deleted (tip ${tip.slice(0, 8)} kept in reflog)` : ''}`,
    fix,
  )
}

// Remote leftovers are the same judgement against origin/main, and OFF BY DEFAULT: deleting a
// remote branch touches everyone who shares this repo, so it takes an explicit --remote.
if (remote) {
  const remoteNames = sh('git for-each-ref --format="%(refname:short)" refs/remotes/origin')
    .split('\n')
    .map((b) => b.trim())
    .filter((b) => b.length > 0 && !b.endsWith('/HEAD') && b !== 'origin/main')
  for (const full of remoteNames) {
    const name = full.replace(/^origin\//, '')
    const cls = branchClassOf({
      name,
      unappliedPatches: unappliedPatchCount(full, 'origin/main'),
      inWorktree: false,
    })
    if (!branchIsDeletable(cls)) continue
    if (!MACHINE_OWNED_BRANCH.test(name)) {
      note('git', `remote branch origin/${name} is landed but is NOT ours (a person named it) — left for you`)
      continue
    }
    if (fix) sh(`git push origin --delete "${name}"`)
    note(
      'git',
      `remote branch origin/${name} is ${cls === 'landed' ? 'fully landed in origin/main' : 'test scaffolding'}${fix ? ' — deleted on origin' : ''}`,
      fix,
    )
  }
}

const clSummary = ['landed', 'scaffolding', 'unlanded']
  .map((k) => `${tally[k] ?? 0} ${k}`)
  .join(' · ')
note(
  'git',
  `local leftover branches: ${clSummary}${remote ? '' : ' (remote untouched — add --remote)'}${fix ? '' : ' — --fix deletes the landed and scaffolding ones'}`,
)

if (deletedTips.length > 0) {
  const log = join(root, '.git', 'forge-branch-prune.log')
  appendFileSync(log, deletedTips.map((line) => line).join('\n') + '\n')
  note('git', `${deletedTips.length} deleted branch tips recorded in .git/forge-branch-prune.log`)
}

// --- conflict copies ANYWHERE in the tree ------------------------------------
// `.next` above is where they cluster, but the one that hides best is the one in source: this repo
// carried `contact-export/contacts-export 2.json`, which looks exactly like a real file. The scan
// skips `.git` (not ours to touch) and `node_modules` (regenerated, enormous).
const SYNC_SKIP = new Set(['.git', 'node_modules', '.next'])
const treeConflicts = conflictCopies(walkFiles(root, [], SYNC_SKIP))
if (treeConflicts.length > 0) {
  if (fix) for (const path of treeConflicts) rmSync(path, { force: true, recursive: true })
  const sample = treeConflicts
    .slice(0, 3)
    .map((p) => p.replace(root + '/', ''))
    .join(', ')
  note(
    'sync',
    `${treeConflicts.length} conflict copy(ies) OUTSIDE .next (${sample}) — cloud-sync debris; the durable fix is moving node_modules/.next out of the synced folder${fix ? ' — removed' : ''}`,
    fix,
  )
} else {
  note('sync', 'no conflict copies in the source tree')
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
  (f) =>
    !f.fixed &&
    /stray|DIRTY|uncommitted|not pushed|landed and clean|fixture and clean|probe\/log|unlanded|left for you|conflict copy/.test(
      f.detail,
    ),
)
console.log(
  remaining.length === 0
    ? '\nthis machine is clear.'
    : `\n${remaining.length} item(s) worth attention — rerun with --fix for the safe ones.`,
)


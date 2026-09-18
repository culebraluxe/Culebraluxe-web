#!/usr/bin/env node
// ---------------------------------------------------------------------------
// RUN A SECTION — not the world.
//
//   pnpm test:section --list                      what exists, and how big each part is
//   pnpm test:section app-money                   one section
//   pnpm test:section FORGE                       every FORGE section (or APP, or HARNESS)
//   pnpm test:changed [--since <ref>]             only the sections your changes can affect
//
// "Changed" is deliberately AREA-level in V1: a path maps to sections by where it lives, not by an
// import graph, because a graph is the inventory job and guessing it would be worse than saying what
// this does. `workflow_app/forge/**` and `agent-runtime/**` are FORGE; `app/`, `components/`, `lib/`,
// `db/` are APP; the harness owns `scripts/`. Tests changed always run their own section.
// ---------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { SECTIONS, areaOf, sectionReport, sectionsForPaths } from './test-sections'

const args = process.argv.slice(2)
const root =
  spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).stdout?.trim() ?? process.cwd()
const sh = (command: string): string =>
  spawnSync(command, { shell: true, cwd: root, encoding: 'utf8' }).stdout?.trim() ?? ''

const report = sectionReport(root)

if (args.includes('--list') || args.length === 0) {
  for (const section of SECTIONS) {
    const files = report.bySection[section.name] ?? []
    console.log(`${section.area.padEnd(8)} ${section.name.padEnd(14)} ${String(files.length).padStart(4)} files  ${section.about}`)
  }
  const byArea = SECTIONS.reduce<Record<string, number>>((acc, s) => {
    acc[s.area] = (acc[s.area] ?? 0) + (report.bySection[s.name]?.length ?? 0)
    return acc
  }, {})
  console.log(
    `\ntotal ${report.total} test files · APP ${byArea.APP ?? 0} · FORGE ${byArea.FORGE ?? 0} · HARNESS ${byArea.HARNESS ?? 0}`,
  )
  if (report.unclassified.length > 0) {
    console.error(`\n${report.unclassified.length} test file(s) unclassified — add a rule in scripts/test-sections.ts:`)
    for (const file of report.unclassified.slice(0, 10)) console.error(`  ? ${file}`)
    process.exit(1)
  }
  process.exit(0)
}

const since = args.includes('--since') ? (args[args.indexOf('--since') + 1] ?? '') : ''
let selected: string[] = []
if (args.includes('--changed')) {
  const diffRange = since ? `${since}...HEAD` : 'HEAD'
  // `git diff` misses UNTRACKED files, which is most of what a working tree is during a sprint: a new
  // fence, a new script. status --porcelain sees added, modified and untracked alike.
  const changed = sh(`git ${since ? `diff --name-only ${diffRange}` : 'status --porcelain'}`)
    .split('\n')
    .map((line) => (since ? line.trim() : line.slice(3).trim()))
    .filter((line) => line.length > 0 && !line.includes(' -> '))
  selected = sectionsForPaths(root, changed)
  console.log(`changed since ${since || 'working tree'}: ${changed.length} file(s)`)
  console.log(`sections to run: ${selected.join(', ') || '(none)'}`)
} else {
  const requested = args.filter((a) => !a.startsWith('--'))
  const names = SECTIONS.map((s) => s.name)
  const areas = ['APP', 'FORGE', 'HARNESS']
  for (const want of requested) {
    if (names.includes(want)) selected.push(want)
    else if (areas.includes(want)) selected.push(...SECTIONS.filter((s) => s.area === want).map((s) => s.name))
    else {
      console.error(`unknown section "${want}" — try: pnpm test:section --list`)
      process.exit(2)
    }
  }
}

const files = selected.flatMap((name) => report.bySection[name] ?? [])
if (files.length === 0) {
  console.log('nothing to run.')
  process.exit(0)
}
const label = selected.map((s) => `${s} (${areaOf(s)})`).join(', ')
console.log(`\nrunning ${files.length} test file(s) for ${label}\n`)
// The env file matters: app sections that read the database need DATABASE_URL_DEV, and the section
// runner is the thing that has to know it rather than every caller remembering.
const envFlag = existsSync(join(root, '.env.local')) ? '--env-file=.env.local ' : ''
const result = spawnSync(
  `node ${envFlag}--import tsx --test ${files.map((f) => `"${f}"`).join(' ')}`,
  {
    shell: true,
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, APP_ENV: process.env.APP_ENV ?? 'development' },
  },
)
process.exit(result.status ?? 1)

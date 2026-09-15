// ---------------------------------------------------------------------------
// forge:manifest — "here are the exact files to read for this scope".
//
//   pnpm forge:manifest PIRATE-01          # write docs/agent/manifest/PIRATE-01.md
//   pnpm forge:manifest PIRATE-01 --check  # exit 1 when the file has drifted
//   pnpm forge:manifest --check-all        # every manifest on disk is fresh
//   pnpm forge:manifest PIRATE-01 --format json
//
// Pirated (idea, not code) from OpenContext's `oc context manifest`, rebuilt so rows
// are RANKED rather than alphabetical and a stale row is a gate failure rather than a
// footnote. Ranking and rendering live in lib/scope-manifest.ts.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

import { writeIfChanged as writeArtifactIfChanged } from '../lib/artifact-file'

import {
  manifestFileName,
  rankManifestEntries,
  renderManifest,
  type ManifestEntry,
  type ManifestMeta,
} from '../lib/scope-manifest'

const MANIFEST_DIR = 'docs/agent/manifest'
const HISTORY_DEPTH = 400

function repoRoot(): string {
  try {
    return execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
  } catch {
    return process.cwd()
  }
}

export function git(args: string[], root: string): string {
  try {
    return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 })
  } catch {
    return ''
  }
}

/**
 * One `git log` pass answers three questions: when each path was last touched, which
 * files the scope's own commits changed, and how many such commits there were.
 * A per-path `git log -1` would be ~130 subprocesses for the same answer.
 */
export function readHistory(
  root: string,
  scope: string,
): { lastTouched: Record<string, string>; scopePaths: string[]; scopeCommits: number } {
  const raw = git(['log', '--pretty=format:__C__%cs%x1f%s', '--name-only', '-n', String(HISTORY_DEPTH)], root)
  const lastTouched: Record<string, string> = {}
  const scopePaths = new Set<string>()
  let scopeCommits = 0
  let date = ''
  let matchesScope = false
  for (const line of raw.split('\n')) {
    if (line.startsWith('__C__')) {
      const [when, subject = ''] = line.slice(5).split('\x1f')
      date = when?.trim() ?? ''
      matchesScope = scope.length > 0 && subject.toUpperCase().includes(scope.toUpperCase())
      if (matchesScope) scopeCommits += 1
      continue
    }
    const path = line.trim()
    if (!path) continue
    if (!lastTouched[path]) lastTouched[path] = date
    if (matchesScope) scopePaths.add(path)
  }
  return { lastTouched, scopePaths: [...scopePaths], scopeCommits }
}

function walkMarkdown(root: string, dir: string): Array<{ path: string; content: string }> {
  const out: Array<{ path: string; content: string }> = []
  const full = join(root, dir)
  if (!existsSync(full)) return out
  for (const name of readdirSync(full)) {
    const child = join(full, name)
    if (statSync(child).isDirectory()) {
      if (name === 'manifest' || name === 'scratch') continue
      out.push(...walkMarkdown(root, join(dir, name)))
      continue
    }
    if (!name.endsWith('.md')) continue
    out.push({ path: relative(root, child), content: readFileSync(child, 'utf8') })
  }
  return out
}

/**
 * The lexical candidate set. Deliberately a plain lexical restriction before any
 * scoring: identifiers are exact-match queries, and the corpus is small enough that
 * the whole harness tree is a fine candidate set.
 */
export function loadCorpus(root: string, scope: string): Array<{ path: string; content: string }> {
  const files = walkMarkdown(root, 'docs/agent')
  if (existsSync(join(root, 'AGENTS.md'))) {
    files.push({ path: 'AGENTS.md', content: readFileSync(join(root, 'AGENTS.md'), 'utf8') })
  }
  const dirScope = scope.includes('/') ? scope.replace(/\/+$/, '') : null
  if (!dirScope) return files
  return files.filter((file) => file.path.startsWith(dirScope))
}

function topLevelPages(root: string): string[] {
  const out: string[] = []
  for (const name of readdirSync(join(root, 'docs/agent'))) {
    if (name.endsWith('.md')) out.push(`docs/agent/${name}`)
  }
  return out.sort()
}

const docsBasenameCache = new Map<string, Map<string, string[]>>()

function docsBasenameIndex(root: string): Map<string, string[]> {
  const cached = docsBasenameCache.get(root)
  if (cached) return cached
  const index = new Map<string, string[]>()
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const full = join(dir, name)
      if (statSync(full).isDirectory()) {
        walk(full)
        continue
      }
      const list = index.get(name) ?? []
      list.push(relative(root, full))
      index.set(name, list)
    }
  }
  try {
    walk(join(root, 'docs'))
  } catch {
    /* no docs tree: nothing to resolve against */
  }
  docsBasenameCache.set(root, index)
  return index
}

/**
 * Is this cited path real?
 *
 * Packets cite three ways: repo-relative (`scripts/forge-manifest.ts`), doc-relative
 * (`packets/README.md`, meaning `docs/agent/packets/README.md`), and by bare basename. A
 * manifest that reports the second form as missing would be crying wolf on a convention
 * the packets already use — measured on FORGE-GATES-01, where `packets/README.md` exists
 * and was being reported as gone. Only a path that resolves nowhere is a finding.
 */
export function resolveManifestPath(root: string, path: string): boolean {
  if (existsSync(join(root, path))) return true
  if (existsSync(join(root, 'docs/agent', path))) return true
  const base = path.slice(path.lastIndexOf('/') + 1)
  return (docsBasenameIndex(root).get(base) ?? []).length === 1
}

/**
 * Does this packet DECLARE THE PATH AS NEW — a deliverable of this story that does not exist yet?
 *
 * The marker is mechanical: the path and `(new)` on the same line. A story's own deliverables do not exist
 * on the base ref — that is what "build this" means — and reporting them as MISSING rows made
 * `manifest-cites-missing-path` red-light the harness on exactly the stories that had not been built yet,
 * the ones that need the gates most (measured 2026-09-15 on ENG-FORGE-DOCTOR-01: three rows, every one of
 * them a file the story exists to create).
 *
 * Exported because a row is a path you can OPEN, and so the rule has ONE definition here rather than a
 * second opinion wherever rows are read. The rows for pending deliverables are dropped below; the lint
 * never has to know why, because it is never handed a path it must resolve and cannot.
 */
export function declaresNew(packetContent: string, path: string): boolean {
  if (!packetContent || !path || path.includes('\n')) return false
  return packetContent
    .split('\n')
    .some((line) => line.includes(path) && /\(\s*new\s*\)/i.test(line))
}

export type BuiltManifest = {
  scope: string
  file: string
  entries: ManifestEntry[]
  meta: ManifestMeta
  markdown: string
}

export function buildManifest(
  root: string,
  scope: string,
  options: { lexicalLimit?: number } = {},
): BuiltManifest {
  const packetPath = `docs/agent/packets/${scope}.md`
  const hasPacket = existsSync(join(root, packetPath))
  const packetContent = hasPacket ? readFileSync(join(root, packetPath), 'utf8') : ''
  const { lastTouched, scopePaths, scopeCommits } = readHistory(root, scope)
  // A packet that documents its own manifest (`docs/agent/manifest/<scope>.md`, which
  // PIRATE-01 does) must not report that file missing on the very first run: we are
  // writing it in this command. Handling it here rather than special-casing the packet
  // keeps the rule "a missing row is a bug" true for every other path.
  const ownFile = `${MANIFEST_DIR}/${manifestFileName(scope)}.md`
  const ownsFile = (path: string) => path === ownFile
  // A cited line may be a COMMAND, not a path (`node --import tsx --test path/to/x.test.ts`). The lint
  // already extracts the path out of it; if the generator does not, the two disagree about the same row
  // again — one says MISSING, the other says fine (2026-09-15, on the Assay line of a new packet).
  const exists = (path: string) =>
    ownsFile(path) ||
    resolveManifestPath(root, path) ||
    path
      .split(/\s+/)
      .some((token) => token.includes('/') && resolveManifestPath(root, token))

  let entries: ManifestEntry[]
  if (hasPacket || scope.includes('/')) {
    entries = rankManifestEntries({
      scope,
      packetPath: hasPacket ? packetPath : null,
      packetContent,
      commitPaths: scopePaths,
      commitCount: scopeCommits,
      lastTouched,
      corpus: loadCorpus(root, scope),
      exists,
      lexicalLimit: options.lexicalLimit,
    })
  } else {
    // No packet and no directory: this is the "what is in the harness" scope. List the
    // handbook and the index pages rather than pretending a ranking exists.
    const rows = rankManifestEntries({
      scope,
      lastTouched,
      exists,
      corpus: [],
    })
    const seen = new Set(rows.map((entry) => entry.path))
    for (const path of topLevelPages(root)) {
      if (seen.has(path)) continue
      rows.push({
        path,
        lane: 'index',
        detail: 'top-level harness page',
        lastTouched: lastTouched[path] ?? null,
        missing: false,
        score: 0,
      })
    }
    entries = rows
  }

  // A row is a path you can OPEN. A deliverable the packet declares `(new)` cannot be opened yet, so it is
  // not a row — and because it is not a row, the lint is never handed a path it must resolve and cannot.
  // The rule has one definition (`declaresNew` above) instead of two opinions, which is what broke on
  // 2026-09-15: the generator said MISSING and the lint said a lie, and between them the harness was red for
  // the very story that was waiting to be built.
  const pending = entries.filter((entry) => declaresNew(packetContent, entry.path)).map((entry) => entry.path)
  entries = entries.filter((entry) => !pending.includes(entry.path))

  const commit = git(['rev-parse', '--short', 'HEAD'], root).trim() || 'unknown'
  const branch = git(['rev-parse', '--abbrev-ref', 'HEAD'], root).trim() || 'unknown'
  const dirty = git(['status', '--porcelain'], root).trim().length > 0
  const meta: ManifestMeta = {
    scope,
    generatedAt: `${new Date().toISOString().slice(0, 19).replace('T', ' ')}Z`,
    commit,
    branch,
    dirty,
    command: `pnpm forge:manifest ${scope}`,
  }
  return {
    scope,
    file: `${MANIFEST_DIR}/${manifestFileName(scope)}.md`,
    entries,
    meta,
    markdown: renderManifest(entries, meta),
  }
}

/**
 * Row-level diff between the file on disk and a fresh render.
 *
 * STRUCTURAL LANES ONLY, by default. Measured the first time it mattered: Grok landed
 * `docs/agent/packets/ENG-FORGE-FACTORY-01.md` (282 lines, one file, no overlap with any
 * file this story touched) and the lexical tail of FORGE-GATES-01's manifest moved — so the
 * gate went red on a commit that had nothing to do with that story. A gate that fails on a
 * co-worker's docs commit is a gate someone switches off, and the value of the freshness
 * check is the part that actually rots: did the packet's cited paths change, did a row's
 * file disappear, did the story's own commits touch something new. The lexical tail is a
 * convenience list and is reported separately, as information.
 *
 * Safety is unchanged either way: a row pointing at a path that is gone still fails the
 * packet lint (rule 8), which parses rows rather than comparing renders.
 */
export function manifestDrift(
  onDisk: string,
  fresh: string,
  options: { lanes?: 'structural' | 'all' } = {},
): { added: string[]; removed: string[] } {
  const lanes = options.lanes ?? 'structural'
  const rows = (markdown: string) =>
    markdown
      .split('\n')
      .filter((line) => line.startsWith('- `'))
      .filter((line) => lanes === 'all' || !isLexicalRow(line))
      .map((line) => line.slice(0, line.lastIndexOf(' · last touched')))
  const before = rows(onDisk)
  const after = rows(fresh)
  const beforeSet = new Set(before)
  const afterSet = new Set(after)
  return {
    added: after.filter((row) => !beforeSet.has(row)),
    removed: before.filter((row) => !afterSet.has(row)),
  }
}

/** A manifest row whose lane is `lexical` — the corpus-dependent tail. */
export function isLexicalRow(line: string): boolean {
  return / — lexical · /.test(line)
}

/** How many lexical rows a fresh render would change: reported, never failed. */
export function lexicalDriftCount(onDisk: string, fresh: string): number {
  const drift = manifestDrift(onDisk, fresh, { lanes: 'all' })
  return drift.added.length + drift.removed.length
}

export function writeIfChanged(path: string, content: string): boolean {
  return writeArtifactIfChanged(path, content)
}

function listManifestFiles(root: string): string[] {
  const dir = join(root, MANIFEST_DIR)
  if (!existsSync(dir)) return []
  return readdirSync(dir)
    .filter((name) => name.endsWith('.md'))
    .map((name) => `${MANIFEST_DIR}/${name}`)
    .sort()
}

function scopeFromManifestFile(file: string): string {
  return file.replace(/^.*\//, '').replace(/\.md$/, '')
}

export type Options = { check: boolean; checkAll: boolean; json: boolean; lexicalLimit?: number; scope: string }

export function parseArgs(argv: string[]): Options {
  const options: Options = { check: false, checkAll: false, json: false, scope: '' }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--check') options.check = true
    else if (arg === '--check-all') options.checkAll = true
    else if (arg === '--format') {
      options.json = argv[i + 1] === 'json'
      i += 1
    } else if (arg === '--lexical') {
      options.lexicalLimit = Number(argv[i + 1])
      i += 1
    } else if (!arg.startsWith('--')) options.scope = arg
  }
  if (!options.scope && !options.checkAll) options.scope = 'all'
  return options
}

function checkOne(root: string, built: BuiltManifest, json: boolean): boolean {
  const path = join(root, built.file)
  const onDisk = existsSync(path) ? readFileSync(path, 'utf8') : ''
  const drift = manifestDrift(onDisk, built.markdown)
  const fresh = onDisk !== '' && drift.added.length === 0 && drift.removed.length === 0
  // Reported, never failed: the lexical tail follows the whole harness corpus, so every
  // docs commit in this repo can move it. Regenerate when you want the tail current.
  const lexical = onDisk === '' ? 0 : lexicalDriftCount(onDisk, built.markdown)
  if (json) {
    console.log(JSON.stringify({ file: built.file, fresh, drift, lexicalRowsBehind: lexical }, null, 2))
    return fresh
  }
  if (fresh) {
    console.log(
      `ok    ${built.file} (${built.entries.length} rows)` +
        (lexical > 0 ? `\n      lexical tail: ${lexical} row(s) behind a fresh render (informational)` : ''),
    )
    return true
  }
  console.log(
    `FAIL  ${built.file}${onDisk === '' ? ' does not exist' : ' has drifted from a fresh render'}` +
      `\n      ${drift.added.length} row(s) would be added, ${drift.removed.length} removed` +
      `\n      regenerate: ${built.meta.command}`,
  )
  for (const row of drift.added.slice(0, 5)) console.log(`      + ${row}`)
  for (const row of drift.removed.slice(0, 5)) console.log(`      - ${row}`)
  return false
}

function main(): number {
  const root = repoRoot()
  const options = parseArgs(process.argv.slice(2))

  if (options.checkAll) {
    const files = listManifestFiles(root)
    if (files.length === 0) {
      console.log('forge:manifest — no manifests on disk yet (run: pnpm forge:manifest PIRATE-01)')
      return 0
    }
    let ok = true
    let totalRows = 0
    for (const file of files) {
      const built = buildManifest(root, scopeFromManifestFile(file), { lexicalLimit: options.lexicalLimit })
      totalRows += built.entries.length
      ok = checkOne(root, { ...built, file }, options.json) && ok
    }
    if (!options.json) {
      console.log(
        `\nforge:manifest — ${files.length} manifest(s), ${totalRows} rows, ${ok ? 'all fresh' : 'DRIFTED'}`,
      )
    }
    return ok ? 0 : 1
  }

  const built = buildManifest(root, options.scope, { lexicalLimit: options.lexicalLimit })
  if (options.check) return checkOne(root, built, options.json) ? 0 : 1

  if (options.json) {
    console.log(
      JSON.stringify(
        {
          scope: built.scope,
          file: built.file,
          meta: built.meta,
          rows: built.entries.length,
          missing: built.entries.filter((entry) => entry.missing).map((entry) => entry.path),
          entries: built.entries,
        },
        null,
        2,
      ),
    )
    return built.entries.some((entry) => entry.missing) ? 1 : 0
  }

  const changed = writeIfChanged(join(root, built.file), built.markdown)
  const byLane = built.entries.reduce<Record<string, number>>((acc, entry) => {
    acc[entry.lane] = (acc[entry.lane] ?? 0) + 1
    return acc
  }, {})
  console.log(`${changed ? 'wrote' : 'unchanged'}  ${built.file}`)
  console.log(
    `  rows: ${built.entries.length}  (${Object.entries(byLane)
      .map(([lane, count]) => `${lane} ${count}`)
      .join(', ')})`,
  )
  const missing = built.entries.filter((entry) => entry.missing)
  if (missing.length > 0) {
    console.log('  MISSING paths — a row that lies is a bug; fix or remove the reference:')
    for (const entry of missing) console.log(`    ${entry.path}  (${entry.lane})`)
    return 1
  }
  console.log('  open the rows top-down; the why column says why each one is here.')
  return 0
}

// The CLI entry, matched on the exact basename so this module's test file does not
// call process.exit inside the test runner.
if (process.argv[1] && /(^|\/)forge-manifest\.ts$/.test(process.argv[1])) {
  process.exit(main())
}

/**
 * RUST PARITY LEDGER — generated evidence, not an opinion.
 *
 *   node --import tsx scripts/rust-parity-ledger.ts      # writes docs/rust-parity-ledger.md
 *
 * WHY: "how far along is the Rust port" deserves a number that comes from the repository rather than from a
 * summary. This walks the import graph from the entry points that actually ship, then reports for every TypeScript
 * module under `db/` and `services/` whether shipped code can still reach it — printed beside the live Rust route
 * table so the two surfaces are visible in one place.
 *
 * WHAT IT DOES NOT PROVE — read this before deleting anything:
 *   * Reachability is not behaviour. A module can be reachable and dead, and it can be unreachable yet still be
 *     the production path (a dynamic `import(someString)` or a registry built from strings is invisible here).
 *   * "Candidate" means "no shipped file imports it", which is the START of a review, not the end. The referencers
 *     are printed next to each candidate precisely so a human can see whether the only caller is a test, a script,
 *     or nothing at all.
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// Run from the repository root regardless of where the caller stands. Every path below is relative — `scripts/`,
// `docs/`, `db/`, `rust/` — and invoking this from `rust/` used to fail with ENOENT on its own map file. Since the
// parity test shells out to this script, the cwd cannot be assumed.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
if (process.cwd() !== ROOT) process.chdir(ROOT)

const ROOTS = ['app', 'components', 'lib', 'workflow_app', 'scripts']
const SUBJECTS = ['db', 'services']
const SKIP = new Set(['node_modules', '.next', '.vercel', '.git', 'target'])
const EXTS = ['', '.ts', '.tsx', '/index.ts', '/index.tsx']

function walk(dir: string, out: string[] = []): string[] {
  let entries: string[]
  try {
    entries = readdirSync(dir)
  } catch {
    return out
  }
  for (const entry of entries) {
    if (SKIP.has(entry)) continue
    const full = join(dir, entry)
    const info = statSync(full)
    if (info.isDirectory()) walk(full, out)
    else if (/\.(ts|tsx)$/.test(entry)) out.push(full)
  }
  return out
}

function specs(file: string): string[] {
  const text = readFileSync(file, 'utf8')
  const found = new Set<string>()
  for (const match of text.matchAll(/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g)) found.add(match[1])
  for (const match of text.matchAll(/require\(\s*['"]([^'"]+)['"]\s*\)/g)) found.add(match[1])
  return [...found]
}

function resolveSpec(spec: string, fromFile: string): string | null {
  const base =
    spec.startsWith('@/')
      ? resolve(spec.slice(2))
      : spec.startsWith('.')
        ? resolve(dirname(fromFile), spec)
        : null
  if (base === null) return null
  for (const ext of EXTS) {
    const candidate = base + ext
    try {
      if (statSync(candidate).isFile()) return candidate
    } catch {
      // try the next extension
    }
  }
  return null
}

const roots = ROOTS.map((root) => resolve(root))
const allFiles = roots.flatMap((root) => walk(root))
// Test files are excluded from the subjects on purpose: nothing IMPORTS a test, so counting them as "unreachable"
// would fill the review queue with false candidates. The first run of this ledger did exactly that — all three
// candidates were *.test.ts files under services/regrid/.
const subjects = SUBJECTS.flatMap((dir) => walk(resolve(dir))).filter(
  (file) => !/\.(test|spec)\.tsx?$/.test(file),
)

// Reachability: breadth-first from the shipped roots.
const reachable = new Set<string>()
const queue = [...allFiles]
while (queue.length > 0) {
  const file = queue.pop() as string
  if (reachable.has(file)) continue
  reachable.add(file)
  for (const spec of specs(file)) {
    const target = resolveSpec(spec, file)
    if (target !== null && !reachable.has(target)) queue.push(target)
  }
}

// Referencers: who mentions each subject module, so a candidate can be judged rather than trusted.
const referencers = new Map<string, string[]>()
for (const file of [...allFiles, ...subjects]) {
  for (const spec of specs(file)) {
    const target = resolveSpec(spec, file)
    if (target === null) continue
    referencers.set(target, [...(referencers.get(target) ?? []), relative(file)])
  }
}

function relative(file: string): string {
  return file.replace(`${process.cwd()}/`, '')
}

const routes = (() => {
  try {
    const text = readFileSync(resolve('rust/server/src/api/routes.rs'), 'utf8')
    // Whitespace-tolerant on purpose: `cargo fmt` reflows a long `.route(...)` onto its own lines, and an earlier
    // version of this regex required `.route("` unbroken — so two mounted routes were reported as missing and the
    // ledger flagged the map for a fault that was the reader's.
    return [...text.matchAll(/\.route\(\s*"([^"]+)"/g)].map((match) => match[1])
  } catch {
    return []
  }
})()

const candidates = subjects.filter((file) => !reachable.has(file))
const reached = subjects.filter((file) => reachable.has(file))

// ---- the pairing map, verified rather than trusted ------------------------------------------------------------
type Capability = {
  id: string
  rustStatus: 'built' | 'partial' | 'not-started'
  productionPath: 'typescript' | 'rust' | 'shadow'
  routes: string[]
  rustModules: string[]
  tsFiles: string[]
  note?: string
}
const map = JSON.parse(readFileSync(resolve('scripts/rust-parity-map.json'), 'utf8')) as {
  capabilities: Capability[]
  knownUnmappedRoutes: string[]
}

const problems: string[] = []
const routeSet = new Set(routes)
const exists = (path: string): boolean => {
  try {
    statSync(resolve(path))
    return true
  } catch {
    return false
  }
}

for (const capability of map.capabilities) {
  for (const route of capability.routes) {
    if (!routeSet.has(route)) {
      problems.push(`${capability.id}: route ${route} is not mounted in the Rust router`)
    }
  }
  for (const module of capability.rustModules) {
    if (!exists(module)) problems.push(`${capability.id}: rust module ${module} does not exist`)
  }
  for (const file of capability.tsFiles) {
    if (!exists(file)) problems.push(`${capability.id}: TS file ${file} does not exist`)
  }
}

const mappedRoutes = new Set(map.capabilities.flatMap((capability) => capability.routes))
const unmappedRoutes = routes.filter(
  (route) => !mappedRoutes.has(route) && !map.knownUnmappedRoutes.includes(route),
)
for (const route of unmappedRoutes) {
  problems.push(`route ${route} is mounted but belongs to no capability — add it to scripts/rust-parity-map.json`)
}

const count = (status: Capability['rustStatus']): number =>
  map.capabilities.filter((capability) => capability.rustStatus === status).length
const liveInRust = map.capabilities.filter((capability) => capability.productionPath === 'rust').length

const lines: string[] = [
  '# Rust parity ledger',
  '',
  'Generated by `node --import tsx scripts/rust-parity-ledger.ts` — do not hand-edit. The run FAILS if the map and',
  'the repository disagree, so a rename or a new route forces this file to be regenerated instead of drifting.',
  '',
  '## Capabilities: what is built in Rust vs what serves production',
  '',
  '`rustStatus` is what exists in Rust. `productionPath` is what actually serves production. They are separate',
  'questions on purpose: a port can be complete and still cut over to nothing.',
  '',
  '| capability | rust | serving production | routes | TS files still in play |',
  '| --- | --- | --- | --- | --- |',
  ...map.capabilities.map(
    (capability) =>
      `| \`${capability.id}\` | ${capability.rustStatus} | ${capability.productionPath} | ` +
      `${capability.routes.length} | ${capability.tsFiles.length ? capability.tsFiles.map((f) => `\`${f}\``).join(', ') : '—'} |`,
  ),
  '',
  `**${count('built')} built · ${count('partial')} partial · ${count('not-started')} not started** — and`,
  `**${liveInRust} of ${map.capabilities.length} capabilities** have Rust as the production path.`,
  '',
  ...map.capabilities
    .filter((capability) => capability.note !== undefined)
    .flatMap((capability) => [`- **${capability.id}** — ${capability.note}`]),
  '',
  '## The live Rust surface',
  '',
  `${routes.length} routes mounted (read from the router, not from this file):`,
  '',
  ...routes.map((route) => `- \`${route}\`${map.knownUnmappedRoutes.includes(route) ? ' _(infrastructure)_' : ''}`),
  '',
  '## TypeScript modules under the subjects',
  '',
  `- \`db/\` and \`services/\`: **${subjects.length}** modules`,
  `- reachable from shipped code: **${reached.length}**`,
  `- not imported by any shipped file (REVIEW, not delete): **${candidates.length}**`,
  '',
  '### Review candidates',
  '',
  ...(candidates.length === 0
    ? ['(none)']
    : candidates.flatMap((file) => {
        const who = referencers.get(file) ?? []
        const from = who.length === 0 ? '**nothing imports it, not even a test**' : who.join(', ')
        return [`- \`${relative(file)}\` — referenced by: ${from}`]
      })),
  '',
  '## Consistency',
  '',
  ...(problems.length === 0 ? ['No drift: every mapped route and path exists, every mounted route is claimed.'] : problems.map((p) => `- ${p}`)),
  '',
  '## What this cannot tell you',
  '',
  '- A module reached only through a dynamic string import looks unreachable here; check the route and message',
  '  registries by hand before deleting.',
  `- \`productionPath\` is a hand-maintained claim in \`scripts/rust-parity-map.json\`, not something this script can`,
  '  observe. Only a receipt showing the Rust path served real traffic can change it.',
  '',
]

const rendered = `${lines.join('\n')}\n`
const docPath = resolve('docs/rust-parity-ledger.md')
const checkOnly = process.argv.includes('--check')

if (checkOnly) {
  // `--check` is the fence: it writes nothing and fails when the committed ledger no longer matches the repository.
  let existing: string
  try {
    existing = readFileSync(docPath, 'utf8')
  } catch {
    existing = ''
  }
  if (existing !== rendered) {
    problems.push('docs/rust-parity-ledger.md is stale — regenerate with node --import tsx scripts/rust-parity-ledger.ts')
  }
} else {
  writeFileSync(docPath, rendered)
}

process.stdout.write(
  `rust-parity-ledger: ${map.capabilities.length} capabilities (${count('built')} built, ${liveInRust} serving in Rust), ` +
    `${subjects.length} TS subject modules, ${reached.length} reachable, ${candidates.length} review candidates, ` +
    `${routes.length} Rust routes, ${problems.length} consistency problem(s)${checkOnly ? ' [check]' : ''}\n`,
)
if (problems.length > 0) process.exitCode = 1


// ---------------------------------------------------------------------------
// FORGE PACKET LINT — scan the HARNESS, not the app.
//
// The useful half of the ECC intake was never its 286 skills; it was the idea that the things which
// steer an agent (packets, skill packs, the memory file, the allowlists) can be checked the same way
// code is. A packet that names a skill that does not exist, or that tells Inspector to commit, is a
// defect in the harness - and harness defects are silent, because nothing runs them.
//
// SCOPE IS DELIBERATELY NARROW: the packet/skill/memory files and the allowlists. It does not walk the
// monorepo, it does not look at application code, and it does not judge prose. Findings are either
// `fail` (exit 1) or `warn` (reported, exit 0), so drift that pre-dates this lint does not block work.
//
//   pnpm forge:packet-lint
// ---------------------------------------------------------------------------

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

import { KNOWN_SKILLS } from '../agent-runtime/skills'
import {
  MANAGED_VENDOR_FILES,
  hasVendorBlock,
  orphanedGuardrails,
  vendorBlockDrifted,
} from '../lib/agent-vendor-block'
import { lineCitations } from '../lib/scope-manifest'
import { DECISION_STATUSES, isValidDecisionKey, parseDecisionFile, validateStatement } from '../lib/forge-decision'
import { SECRET_SHAPES } from '../lib/secret-shapes'

export type Finding = {
  level: 'fail' | 'warn'
  rule: string
  file: string
  line?: number
  message: string
}

export type HarnessFile = { path: string; content: string }

/**
 * DEBT, RECORDED. A finding whose `path::rule` key is here is reported as a warning instead of failing.
 *
 * Two gates in this story share one principle: a NEW violation blocks, pre-existing debt is reported.
 * Without that, day one is 8 failures in four older packets and the lint gets disabled before it has
 * ever caught anything - which is how the rules in AGENTS.md became sentences in the first place.
 * Keys are `path::rule` without line numbers, so editing a file above a known finding does not
 * "un-baseline" it.
 */
export type LintBaseline = { findings?: string[] }

export function baselineKey(finding: Pick<Finding, 'file' | 'rule'>): string {
  return `${finding.file}::${finding.rule}`
}


const ROLES_THAT_MAY_NOT_COMMIT = /\b(scout|assay|inspector)\b/i

/**
 * Is this a DIRECTIVE telling a role to commit, or prose about commits?
 *
 * Measured against real text on 2026-09-15, the naive "role + commit" rule flagged all of these, none of
 * which is an instruction:
 *   - "any instruction telling Scout, Assay or Inspector to `git commit`"  (describing this very rule)
 *   - "Only `builder` may keep a git commit or write DEV. Scout/Assay commits are rewinded."  (a rule)
 *   - "Never create a git commit as Scout, Assay or Inspector."  (a prohibition)
 * A gate that fires on the rule's own documentation is a gate someone disables. So the role must be the
 * SUBJECT of the instruction, not a word inside a sentence about one.
 */
function isCommitDirective(window: string): boolean {
  if (!ROLES_THAT_MAY_NOT_COMMIT.test(window)) return false
  if (PROHIBITION.test(window)) return false
  // The role opens the instruction: "Inspector: verify, then git commit", "- Inspector commits the fix".
  if (/^(?:then\s+|and\s+|next,?\s+|-\s*)?(?:scout|assay|inspector)\b[^.\n]{0,60}?\bcommit/i.test(window.trim())) {
    return true
  }
  // The role is handed the act: "then have Inspector commit", "after Inspector commits".
  return /\b(?:then|after|have|let)\s+(?:scout|assay|inspector)\s+(?:commit|commits|should commit|must commit|will commit)/i.test(
    window,
  )
}

const COMMIT_INSTRUCTION = /git commit|create (?:a )?commit|--no-verify/i
/** A prohibition is not an instruction: "Never ... a git commit as Scout" is the rule, not a violation. */
const PROHIBITION = /\b(?:never|not|no|may not|must not|do not|don't|cannot|can't|forbidden|prohibited)\b/i

const MAX_SKILLS_PER_PACKET = 3

function headingsIn(content: string): Array<{ name: string; line: number; body: string }> {
  const lines = content.split('\n')
  const out: Array<{ name: string; line: number; body: string }> = []
  let current: { name: string; line: number; body: string } | null = null
  for (let i = 0; i < lines.length; i += 1) {
    const match = /^##\s+(.+?)\s*$/.exec(lines[i])
    if (match) {
      if (current) out.push(current)
      current = { name: match[1].trim(), line: i + 1, body: '' }
      continue
    }
    if (current) current.body += `${lines[i]}\n`
  }
  if (current) out.push(current)
  return out
}

/**
 * Repo paths a MAP cites must exist.
 *
 * This is what separates a map from prose: a map that points at a file that moved is worse than no map,
 * because it wastes the reader's time and teaches them to distrust it. Applied to the orientation/map
 * pages only (ORIENTATION.md, MAP-*.md) - the historical log is allowed to mention files that are gone,
 * because that is what history is.
 */
function citedRepoPaths(content: string): string[] {
  const out: string[] = []
  for (const match of content.matchAll(/`([A-Za-z0-9_./*<>{}-]+)`/g)) {
    const token = match[1]
    if (!token.includes('/')) continue // not a path (table names, commands without paths)
    if (token.startsWith('/')) continue // a route, not a repo path
    if (/^https?:/.test(token)) continue
    if (token.startsWith('.next/') || token.startsWith('.vercel/') || token.startsWith('.git/')) continue
    if (/[<>{}]/.test(token)) continue // placeholder like services/<domain>/
    if (/\.(md|ts|tsx|mjs|js|json|sql|xml|css)$/.test(token) || token.endsWith('/')) out.push(token)
  }
  return [...new Set(out)]
}

function pathExists(repoRoot: string, token: string): boolean {
  const cleaned = token.replace(/\/$/, '')
  if (cleaned.includes('*')) {
    // A glob: try the directory part, then require at least one match.
    const dir = cleaned.slice(0, cleaned.lastIndexOf('/'))
    const base = cleaned.slice(cleaned.lastIndexOf('/') + 1)
    try {
      const re = new RegExp(`^${base.split('*').map(escapeRegExp).join('.*')}$`)
      return readdirSync(join(repoRoot, dir)).some((name) => re.test(name))
    } catch {
      return false
    }
  }
  try {
    return statSync(join(repoRoot, cleaned)).isFile() || statSync(join(repoRoot, cleaned)).isDirectory()
  } catch {
    return false
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/** Source roots the basename resolver walks. Bounded on purpose: this is a lint, not an indexer. */
const SOURCE_ROOTS = [
  'app',
  'components',
  'lib',
  'services',
  'db',
  'scripts',
  'agent-runtime',
  'workflow_app',
  'ui',
  'testv2',
  'legacy/db/migrations',
]

const basenameCache = new Map<string, Map<string, string[]>>()

/**
 * basename -> repo-relative paths, so a packet's shorthand citation
 * (`projects-workspace.tsx:639-656`) can still be verified.
 *
 * Measured 2026-09-15: 15 of the 16 hits from the first live run of rule 10 were this
 * shorthand, not stale claims. A gate that fails a writing convention gets switched off,
 * so the resolver exists instead of a stricter rule.
 */
function basenameIndex(repoRoot: string): Map<string, string[]> {
  const cached = basenameCache.get(repoRoot)
  if (cached) return cached
  const index = new Map<string, string[]>()
  const walk = (dir: string, depth: number) => {
    if (depth > 10) return
    let entries: string[]
    try {
      entries = readdirSync(dir)
    } catch {
      return
    }
    for (const name of entries) {
      if (name === 'node_modules' || name.startsWith('.')) continue
      const full = join(dir, name)
      let isDirectory = false
      try {
        isDirectory = statSync(full).isDirectory()
      } catch {
        continue
      }
      if (isDirectory) {
        walk(full, depth + 1)
        continue
      }
      const list = index.get(name) ?? []
      list.push(relative(repoRoot, full))
      index.set(name, list)
    }
  }
  for (const root of SOURCE_ROOTS) walk(join(repoRoot, root), 0)
  basenameCache.set(repoRoot, index)
  return index
}

type ResolvedCitation = { kind: 'file'; path: string } | { kind: 'none' } | { kind: 'ambiguous' }

/**
 * Packets and manifests cite three ways: repo-relative (`scripts/forge-manifest.ts`),
 * doc-relative (`packets/README.md`, meaning `docs/agent/packets/README.md`), and by bare
 * basename. One resolver handles all three, and rules 8 and 10 share it — measured
 * 2026-09-15, they disagreed while each had its own copy, and the gate reported a path as
 * missing that the generator had just resolved.
 */
function resolveCitation(repoRoot: string, path: string): ResolvedCitation {
  if (pathExists(repoRoot, path)) return { kind: 'file', path }
  if (path.includes('/')) {
    const docRelative = join('docs/agent', path)
    if (pathExists(repoRoot, docRelative)) return { kind: 'file', path: docRelative }
  }
  const matches = basenameIndex(repoRoot).get(path.replace(/^.*\//, '')) ?? []
  if (matches.length === 1) return { kind: 'file', path: matches[0] }
  return matches.length === 0 ? { kind: 'none' } : { kind: 'ambiguous' }
}

/** `content.split('\n').length` is one too many for a file ending in a newline. */
function countLines(content: string): number {
  const lines = content.split('\n')
  return content.endsWith('\n') ? lines.length - 1 : lines.length
}

function isMapPage(path: string): boolean {
  return /docs\/agent\/(ORIENTATION|MAP-[^/]+)\.md$/.test(path)
}

function isPacket(path: string): boolean {
  return /docs\/agent\/packets\/[^/]+\.md$/.test(path) && !/README\.md$/.test(path)
}


/**
 * True when a list item sits under a prohibition heading (`Never`, `Do not`, `Ask first`).
 *
 * AGENTS.md's Never list is the rule statement, not an instruction: its items read as imperatives
 * ("Keep a git commit as Scout, Assay, or Inspector.") and the only clue is the heading above them.
 */
function prohibitionFor(lines: readonly string[], index: number): boolean {
  for (let i = index; i >= 0 && index - i <= 25; i -= 1) {
    const line = lines[i].trim()
    if (!line) continue
    if (/^[-*]\s+/.test(line)) continue
    if (/^#*\s*(never|do not|don't|must not|forbidden)\b/i.test(line)) return true
    if (/^(never|do not|don't|must not|forbidden)$/i.test(line)) return true
    return false
  }
  return false
}

export function lintHarness(input: {
  files: readonly HarnessFile[]
  knownSkills?: readonly string[]
  baseline?: readonly string[]
  repoRoot?: string
}): Finding[] {
  const known = input.knownSkills ?? KNOWN_SKILLS
  const baselined = new Set(input.baseline ?? [])
  const repoRoot = input.repoRoot ?? process.cwd()
  const findings: Finding[] = []

  for (const file of input.files) {
    const lines = file.content.split('\n')

    // RULE 1 + 2 — a packet's `## Skills` must name real skills, at most three of them.
    if (isPacket(file.path)) {
      const skills = headingsIn(file.content).find((h) => /^skills$/i.test(h.name))
      if (skills) {
        const tokens = skills.body
          .toLowerCase()
          .split(/[\s,]+/)
          .map((t) => t.replace(/^[-*]+/, '').trim())
          .filter(Boolean)
        const unknown = tokens.filter((t) => !known.includes(t))
        if (unknown.length > 0) {
          findings.push({
            level: 'fail',
            rule: 'skills-unknown',
            file: file.path,
            line: skills.line,
            message: `names skill(s) that do not exist: ${unknown.join(', ')} (known: ${known.join(', ')})`,
          })
        }
        if (tokens.length > MAX_SKILLS_PER_PACKET) {
          findings.push({
            level: 'fail',
            rule: 'skills-too-many',
            file: file.path,
            line: skills.line,
            message: `lists ${tokens.length} skills; the cap is ${MAX_SKILLS_PER_PACKET}`,
          })
        }
      }
    }

    // RULE 3 — every dated MEMORY.md ENTRY carries a date, so an undated fact cannot hide in the log.
    // Scoped to entry-style bullets (`- **…`): the file also holds plain bullets that are section
    // content (standing rules, glossary), and demanding a date from those is how a lint becomes noise.
    if (/docs\/agent\/MEMORY\.md$/.test(file.path)) {
      for (let i = 0; i < lines.length; i += 1) {
        const line = lines[i]
        if (!/^[-*]\s+\*\*/.test(line)) continue
        if (!/\d{4}-\d{2}-\d{2}/.test(line.slice(0, 80))) {
          findings.push({
            level: 'fail',
            rule: 'memory-entry-undated',
            file: file.path,
            line: i + 1,
            message: 'entry has no YYYY-MM-DD prefix in its first 80 characters',
          })
        }
      }
    }

    // RULE 4 — no secret-shaped token, anywhere in the harness.
    for (let i = 0; i < lines.length; i += 1) {
      for (const shape of SECRET_SHAPES) {
        if (shape.pattern.test(lines[i])) {
          findings.push({
            level: 'fail',
            rule: 'secret-shape',
            file: file.path,
            line: i + 1,
            message: `looks like a ${shape.name}`,
          })
        }
      }
    }

    // RULE 7 — a MAP must only point at files that exist (see citedRepoPaths above).
    if (isMapPage(file.path)) {
      for (const cited of citedRepoPaths(file.content)) {
        if (pathExists(repoRoot, cited)) continue
        findings.push({
          level: 'fail',
          rule: 'map-cites-missing-path',
          file: file.path,
          message: `cites \`${cited}\`, which does not exist — the map has drifted from the code`,
        })
      }
    }

    // RULE 5 — a role that may not commit must not be told to (that is an AGENTS.md Never).
    //
    // The prohibition can be a SECTION HEADER rather than a word on the line: AGENTS.md lists
    // "Keep a git commit as Scout, Assay, or Inspector." under a `Never` heading, so the cue is up to a
    // dozen lines above the item. Reading only a +/-3 line window flagged the rule statement itself -
    // a false positive that would have taught everyone to ignore the gate.
    if (prohibitionFor(lines, 0)) continue
    for (let i = 0; i < lines.length; i += 1) {
      if (prohibitionFor(lines, i)) continue
      const window = [lines[i], lines[i + 1] ?? '', lines[i + 2] ?? ''].join(' ')
      if (!isCommitDirective(window)) continue
      findings.push({
        level: 'fail',
        rule: 'non-builder-commit-instruction',
        file: file.path,
        line: i + 1,
        message: 'instructs Scout/Assay/Inspector to commit; only the Builder role commits',
      })
    }
  }

  // RULE 6 (warn) — the skill list and the skill files have drifted apart. Discovered 2026-09-15:
  // `docs/agent/skills/` holds nine packs (cruiser, knip, ripwire, rtk, semgrep, serena are not in
  // KNOWN) while `workflow` and `ui` are in KNOWN with no file at all. Reported, not failed: it
  // pre-dates this lint, and a gate that blocks on day one gets switched off on day one.
  const skillFiles = input.files
    .filter((f) => /docs\/agent\/skills\/[^/]+\.md$/.test(f.path))
    .map((f) => f.path.replace(/.*\//, '').replace(/\.md$/, ''))
    .filter((name) => name !== 'README')
  for (const file of skillFiles) {
    if (!known.includes(file)) {
      findings.push({
        level: 'warn',
        rule: 'skill-file-not-in-known',
        file: `docs/agent/skills/${file}.md`,
        message: 'skill file exists but is not in KNOWN_SKILLS, so no packet can load it',
      })
    }
  }
  for (const skill of known) {
    if (!skillFiles.includes(skill)) {
      findings.push({
        level: 'warn',
        rule: 'known-skill-has-no-file',
        file: `docs/agent/skills/${skill}.md`,
        message: `KNOWN_SKILLS lists "${skill}" but no pack file exists to load`,
      })
    }
  }

  // RULE 8 — a generated scope manifest must not point at a path that is gone. Same
  // principle as rule 7: the value of a generated index is that it cannot lie, and a
  // row for a deleted file is a lie the reader cannot see. Parse the rows rather than
  // re-deriving them here: the manifest CLI owns the ranking, the lint owns the claim.
  for (const file of input.files) {
    if (!/docs\/agent\/manifest\/[^/]+\.md$/.test(file.path)) continue
    for (const line of file.content.split('\n')) {
      const row = /^- `([^`]+)`( \*\*MISSING\*\*)? — /.exec(line)
      if (!row) continue
      if (resolveCitation(repoRoot, row[1]).kind === 'none') {
        findings.push({
          level: 'fail',
          rule: 'manifest-cites-missing-path',
          file: file.path,
          message: `row \`${row[1]}\` resolves to no file — regenerate: pnpm forge:manifest <scope>, or fix the reference in the packet`,
        })
      }
    }
  }

  // RULE 9 — the two halves of the vendor-block mechanism (lib/agent-vendor-block.ts):
  // a block that drifted from a fresh render, and a guardrail whose sentence is no longer
  // in AGENTS.md. The second one is the whole point of replicating a rule: a generated
  // file must never assert something the handbook stopped saying.
  const agents = input.files.find((file) => file.path === 'AGENTS.md')
  if (agents) {
    for (const guardrail of orphanedGuardrails(agents.content)) {
      findings.push({
        level: 'fail',
        rule: 'guardrail-anchor-missing',
        file: agents.path,
        message: `AGENTS.md no longer contains "${guardrail.anchoredBy}", so the generated block still asserts it`,
      })
    }
    for (const file of input.files) {
      if (!(MANAGED_VENDOR_FILES as readonly string[]).includes(file.path)) continue
      if (!hasVendorBlock(file.content)) continue
      if (vendorBlockDrifted(file.content)) {
        findings.push({
          level: 'fail',
          rule: 'vendor-block-drift',
          file: file.path,
          message: 'the generated block differs from a fresh render — run: pnpm forge:sync-agents',
        })
      }
    }
  }

  // RULE 10 — evidence cites a file AND a range; a range past the end of the file is a
  // stale claim that reads as precision. Applied to packets and maps, the two places we
  // ask a reader to verify a statement against a specific line.
  for (const file of input.files) {
    if (!isPacket(file.path) && !isMapPage(file.path)) continue
    for (const citation of lineCitations(file.content)) {
      const resolved = resolveCitation(repoRoot, citation.path)
      if (resolved.kind === 'ambiguous') continue // two files share the name; not this gate's call
      if (resolved.kind === 'none') {
        findings.push({
          level: 'fail',
          rule: 'evidence-cites-missing-file',
          file: file.path,
          message: `cites \`${citation.raw}\` but no file named \`${citation.path}\` exists`,
        })
        continue
      }
      let lineCount: number
      try {
        lineCount = countLines(readFileSync(join(repoRoot, resolved.path), 'utf8'))
      } catch {
        continue
      }
      if (citation.end > lineCount) {
        findings.push({
          level: 'fail',
          rule: 'evidence-line-past-eof',
          file: file.path,
          message: `cites \`${citation.raw}\` but \`${resolved.path}\` has ${lineCount} lines`,
        })
      }
    }
  }

  // RULE 11 (warn) — a skill pack is loadable knowledge, so it should anchor to the code
  // it describes. Seven packs are pure prose today; warning rather than failing is the
  // same day-one decision as rule 6, and the count is the number worth watching.
  for (const file of input.files) {
    if (!/docs\/agent\/skills\/[^/]+\.md$/.test(file.path) || /README\.md$/.test(file.path)) continue
    const anchored = citedRepoPaths(file.content).some((path) => pathExists(repoRoot, path))
    if (!anchored) {
      findings.push({
        level: 'warn',
        rule: 'skill-not-anchored',
        file: file.path,
        message: 'names no repo path that exists — the pack describes code it does not point at',
      })
    }
  }

  // RULE 12 — a decision mirror must still LOOK like a decision: the filename is the key, the status is
  // one of three, and the statement is one sentence. Whether the file matches its row needs the database,
  // which this lint deliberately does not touch (`pnpm forge:decision check` owns that half). What can be
  // checked offline is the shape, and shape is what a hand edit breaks first.
  for (const file of input.files) {
    if (!/docs\/agent\/decisions\/[^/]+\.md$/.test(file.path)) continue
    const fileKey = file.path.replace(/^.*\//, '').replace(/\.md$/, '')
    const parsed = parseDecisionFile(file.content, fileKey)
    if (parsed.key !== fileKey) {
      findings.push({
        level: 'fail',
        rule: 'decision-file-key-mismatch',
        file: file.path,
        message: `title says "${parsed.key}" but the filename says "${fileKey}" — the filename IS the key`,
      })
    }
    if (!isValidDecisionKey(parsed.key)) {
      findings.push({
        level: 'fail',
        rule: 'decision-file-key-mismatch',
        file: file.path,
        message: `"${parsed.key}" is not a lowercase slug`,
      })
    }
    if (!DECISION_STATUSES.includes(parsed.status as (typeof DECISION_STATUSES)[number])) {
      findings.push({
        level: 'fail',
        rule: 'decision-file-status',
        file: file.path,
        message: `status "${parsed.status}" is not one of ${DECISION_STATUSES.join(', ')}`,
      })
    }
    for (const problem of validateStatement(parsed.statement)) {
      findings.push({ level: 'fail', rule: 'decision-file-statement', file: file.path, message: problem })
    }
    if (!file.content.includes('GENERATED from forge_decision')) {
      findings.push({
        level: 'warn',
        rule: 'decision-file-not-generated',
        file: file.path,
        message: 'no generated-by marker: hand-written decisions belong in the table, not the mirror',
      })
    }
  }

  // Debt recorded at the baseline is reported, not blocking - see LintBaseline above.
  return findings.map((finding) => {
    if (finding.level !== 'fail' || !baselined.has(baselineKey(finding))) return finding
    return { ...finding, level: 'warn' as const, message: `pre-existing (baselined): ${finding.message}` }
  })
}

/** The harness files, and only the harness files. */
export function loadHarnessFiles(repoRoot = process.cwd()): HarnessFile[] {
  const out: HarnessFile[] = []
  const add = (path: string) => {
    try {
      if (!statSync(path).isFile()) return
      out.push({ path: relative(repoRoot, path), content: readFileSync(path, 'utf8') })
    } catch {
      /* a missing optional file is not a finding */
    }
  }
  const addDir = (dir: string, filter: (name: string) => boolean) => {
    try {
      for (const name of readdirSync(dir)) if (filter(name)) add(join(dir, name))
    } catch {
      /* absent directory */
    }
  }

  add(join(repoRoot, 'AGENTS.md'))
  add(join(repoRoot, 'docs/agent/MEMORY.md'))
  addDir(join(repoRoot, 'docs/agent/packets'), (n) => n.endsWith('.md'))
  addDir(join(repoRoot, 'docs/agent/skills'), (n) => n.endsWith('.md'))
  // The MAP pages: they claim to point at real files, so they are scanned (rule 7 checks the claim).
  addDir(join(repoRoot, 'docs/agent'), (n) => /^(ORIENTATION|MAP-.+)\.md$/.test(n))
  addDir(join(repoRoot, 'agent-runtime'), (n) => n.endsWith('.ts') && !n.endsWith('.test.ts'))
  // Generated scope manifests: each row claims a path on disk (rule 8 checks the claim).
  addDir(join(repoRoot, 'docs/agent/manifest'), (n) => n.endsWith('.md'))
  // Decision mirrors (migration 180): scanned for STRUCTURE only — whether each file still matches its
  // row needs the database, and that check lives in `pnpm forge:decision check` (rule 12 covers shape).
  addDir(join(repoRoot, 'docs/agent/decisions'), (n) => n.endsWith('.md'))
  // Vendor pointer files that carry a generated block (rule 9 checks it has not drifted).
  for (const name of MANAGED_VENDOR_FILES) add(join(repoRoot, name))

  return out
}

/** The recorded debt, if any. Absent file means "no baseline", which fails on everything. */
export function loadBaseline(repoRoot = process.cwd()): string[] {
  try {
    const raw = readFileSync(join(repoRoot, 'docs/agent/harness-lint-baseline.json'), 'utf8')
    const parsed = JSON.parse(raw) as LintBaseline
    return Array.isArray(parsed.findings) ? parsed.findings : []
  } catch {
    return []
  }
}

function main(): number {
  const argv = process.argv.slice(2)
  const json = argv.includes('--format') && argv[argv.indexOf('--format') + 1] === 'json'
  // A DOC GATE MUST NOT BE ABLE TO STOP A RELEASE. The citations this checks are hand-written prose inside old
  // packets, and a release has twice been blocked by stale paths in documents that had nothing to do with the code
  // being shipped. Findings are printed loudly and the exit code is 0; `--strict` restores the old blocking
  // behaviour for anyone who wants it deliberately.
  const strict = argv.includes('--strict')
  const files = loadHarnessFiles()
  const baseline = loadBaseline()
  const findings = lintHarness({ files, baseline })
  const failures = findings.filter((f) => f.level === 'fail')
  const warnings = findings.filter((f) => f.level === 'warn')
  const baselinedCount = warnings.filter((f) => f.message.startsWith('pre-existing (baselined)')).length

  // The machine view. A gate the cockpit or the engine can consume is a gate that can be
  // rendered where the work happens; the human view below stays the default.
  if (json) {
    console.log(
      JSON.stringify(
        {
          filesScanned: files.length,
          failures: failures.length,
          warnings: warnings.length,
          baselined: baselinedCount,
          findings: findings.map((f) => ({ ...f, baselined: f.message.startsWith('pre-existing (baselined)') })),
        },
        null,
        2,
      ),
    )
    return strict && failures.length > 0 ? 1 : 0
  }

  for (const f of findings) {
    const where = f.line ? `${f.file}:${f.line}` : f.file
    console.log(`${f.level === 'fail' ? 'FAIL' : 'warn'}  ${f.rule.padEnd(32)} ${where}\n      ${f.message}`)
  }
  console.log(
    `\nforge:packet-lint — ${failures.length} failure(s), ${warnings.length} warning(s)` +
      ` (${baselinedCount} baselined), ${files.length} harness file(s) scanned` +
      (strict && failures.length > 0 ? '' : ' — reported, not blocking (use --strict to block)'),
  )
  return strict && failures.length > 0 ? 1 : 0
}

// The CLI entry, matched on the exact basename: a prefix match also catches this module's own test file
// (`forge-packet-lint.test.ts`), which would then call process.exit inside the test runner.
if (process.argv[1] && /(^|\/)forge-packet-lint\.ts$/.test(process.argv[1])) {
  process.exit(main())
}


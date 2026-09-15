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

const SECRET_SHAPES: Array<{ name: string; pattern: RegExp }> = [
  { name: 'openai-style key', pattern: /\bsk-[A-Za-z0-9]{16,}\b/ },
  { name: 'github token', pattern: /\bghp_[A-Za-z0-9]{20,}\b/ },
  { name: 'aws access key id', pattern: /\bAKIA[0-9A-Z]{12,}\b/ },
  { name: 'database url with credentials', pattern: /postgres(?:ql)?:\/\/[^\s'"]+:[^\s'"]+@/i },
]

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
  const files = loadHarnessFiles()
  const baseline = loadBaseline()
  const findings = lintHarness({ files, baseline })
  const failures = findings.filter((f) => f.level === 'fail')
  const warnings = findings.filter((f) => f.level === 'warn')
  const baselinedCount = warnings.filter((f) => f.message.startsWith('pre-existing (baselined)')).length

  for (const f of findings) {
    const where = f.line ? `${f.file}:${f.line}` : f.file
    console.log(`${f.level === 'fail' ? 'FAIL' : 'warn'}  ${f.rule.padEnd(32)} ${where}\n      ${f.message}`)
  }
  console.log(
    `\nforge:packet-lint — ${failures.length} failure(s), ${warnings.length} warning(s)` +
      ` (${baselinedCount} baselined), ${files.length} harness file(s) scanned`,
  )
  return failures.length > 0 ? 1 : 0
}

// The CLI entry, matched on the exact basename: a prefix match also catches this module's own test file
// (`forge-packet-lint.test.ts`), which would then call process.exit inside the test runner.
if (process.argv[1] && /(^|\/)forge-packet-lint\.ts$/.test(process.argv[1])) {
  process.exit(main())
}


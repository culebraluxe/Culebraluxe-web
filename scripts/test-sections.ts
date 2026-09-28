#!/usr/bin/env node
// ---------------------------------------------------------------------------
// WHICH TESTS BELONG TO WHAT — the taxonomy, so nobody runs all 3,300 to change a button.
//
// The captain, 2026-09-18: "we should split the tests to be APP tests and Forge tests ... and we might
// even split the test tree down by section so we are not killing time."
//
// WHY THIS IS A MAPPING AND NOT A MOVE. Every story's frozen fence names an exact path
// (`node --import tsx --test workflow_app/tests/claim-clock.test.ts`) and the acceptance mapping binds
// clauses to those names. Physically moving the tree would break 65 stories' proofs at once, so the
// split is expressed as a CLASSIFICATION: every test file belongs to exactly one section, sections
// roll up into APP and FORGE, and the runner selects by section. A future physical move is then a
// mechanical follow-through of this map rather than a leap.
//
// THE DISCIPLINE: every test file must be classified. A new test file with no rule is reported as
// unclassified and `scripts/test-sections.test.ts` fails the harness for it — the same shape as the
// column-writer audit, because an unclassified file is a file nobody will ever run.
// ---------------------------------------------------------------------------
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

export type Area = 'FORGE' | 'APP' | 'HARNESS'

export type Section = {
  name: string
  area: Area
  /** What the section is for, in the words the runner prints. */
  about: string
  /**
   * TRUE when the tree this section named is gone and no test file matches it any more.
   *
   * The port moved the tests beside the code they check: 385 separate TypeScript suite files became ~150
   * Rust files that hold their own `#[cfg(test)]` tests, so six of the ten sections below have no file
   * today. Deleting them would have been the easy lie — they are still how an old story's frozen fence
   * path (`workflow_app/tests/claim-clock.test.ts`) is classified, and `sectionForFile` still answers for
   * those paths. What is NOT allowed is a section that is empty and pretends otherwise: this flag is that
   * admission, `now` says which Rust crate owns the capability, and the gate fails a section that is
   * empty without it.
   */
  historical?: true
  /** For a historical section: where the capability lives now. */
  now?: string
}

export const SECTIONS: Section[] = [
  { name: 'forge-engine', area: 'FORGE', about: 'the engine: waves, dispatch, routing, contracts, gates' },
  {
    name: 'forge-verify',
    area: 'FORGE',
    about: 'verification itself: acceptance, assertions, receipts, migrations',
    historical: true,
    now: 'rust/cli/src/forge/lint.rs — the packet lint is the surviving verification rule set',
  },
  {
    name: 'forge-runtime',
    area: 'FORGE',
    about: 'the agent runtime: adapters, sessions, workspaces, invokers',
    historical: true,
    now: 'rust/forge/src/engine — the engine owns the runtime the TypeScript adapters drove',
  },
  {
    name: 'app-crm',
    area: 'APP',
    about: 'the book of business: clients, deals, documents, deadlines',
    historical: true,
    now: 'rust/core/domain + rust/core/db — the domain is Rust now',
  },
  {
    name: 'app-intake',
    area: 'APP',
    about: 'intake: mail, messages, Apple surfaces, calendar, conversations',
    historical: true,
    now: 'rust/integrations — every provider adapter is there',
  },
  {
    name: 'app-money',
    area: 'APP',
    about: 'money: accounting, banking, statements',
    historical: true,
    now: 'rust/server/src/api — accounting is served by the Rust API',
  },
  {
    name: 'app-identity',
    area: 'APP',
    about: 'identity and access: auth, entitlements, people',
    historical: true,
    now: 'rust/server/src/api + rust/core/db/src/identity_cache.rs',
  },
  { name: 'app-portal', area: 'APP', about: 'the portal surface: navigation, views, boards, readiness' },
  { name: 'app-core', area: 'APP', about: 'shared app core: commands, db seams, contracts' },
  { name: 'harness', area: 'HARNESS', about: 'the guardrails themselves: manifests, packets, protections' },
]

/**
 * ORDERED RULES, FIRST MATCH WINS. Path prefixes first (a tree says what it is), then name patterns.
 * Order matters: `forge-` beats a broad app rule, and a specific word beats a generic one.
 */
export const SECTION_RULES: Array<{ section: string; match: RegExp }> = [
  { section: 'harness', match: /^scripts\// },
  // THE RUST TREE, which is where the product and its tests live since `4cf98110`. The CRATE is the section,
  // because the crate is what a person actually runs (`cargo test -p forge`), and a rule that disagrees with
  // the command people type is a rule they will route around. Forge-first so a forge command under
  // `rust/cli` is the engine's, not the harness's.
  { section: 'forge-engine', match: /^rust\/(cli\/src\/forge|forge)\// },
  { section: 'forge-engine', match: /^rust\/core\/workflow\// },
  { section: 'app-portal', match: /^rust\/ui\// },
  { section: 'app-core', match: /^rust\/(core|server|integrations)\// },
  { section: 'harness', match: /^rust\/cli\// },
  { section: 'app-core', match: /^rust\// },
  { section: 'forge-runtime', match: /^agent-runtime\// },
  { section: 'forge-runtime', match: /^testv2\/engine_tests\/(persistence|hardening|dynamic)/ },
  { section: 'forge-engine', match: /^testv2\/engine_tests\// },
  { section: 'forge-engine', match: /^legacy\/workflow_app\/tests\/forge-/ },
  {
    section: 'forge-engine',
    match:
      /(^|\/)(dynamic-fork|execution-graph|factory-|agent-scheduler|agent-work|finding-dedupe|claim-clock|completion-crash-window|concurrency|column-writer-audit|db-boundary|db-gateway|db-routing)/,
  },
  {
    section: 'forge-verify',
    match:
      /^legacy\/workflow_app\/tests\/(acceptance|assertion-|evidence|migration-|release-|resume-|verify-|stale-|proof-|qa-|receipt|typesafe-|failure-triage)/,
  },
  {
    section: 'app-intake',
    match:
      /(^|\/)(apple-|applemail|whatsapp|calendar-|conversation|intake|mac-observer|eventkit|message|mail|snapshot-|boldson)/,
  },
  { section: 'app-money', match: /(^|\/)(accounting|bank-|ofx|pnl|expense|receivable|financing|appraisal|payment)/ },
  {
    section: 'app-identity',
    match: /(^|\/)(auth|identity|entitlement|people|dev-bypass|secret-binding|address-format|favorite)/,
  },
  {
    section: 'app-crm',
    match:
      /(^|\/)(client|deal|closing|document|deadline|broker|showing|property|participant|signature|agreement|crm|ara-|doc0)/,
  },
  {
    section: 'app-portal',
    match: /(^|\/)(portal|navigation|storyboard|board|surface|readiness|flight-recorder|observe|tech-|views)/,
  },
  { section: 'app-core', match: /^legacy\/workflow_app\/tests\// },
]

/** Names no rule reads correctly, each mapped where it belongs. Kept tiny on purpose. */
export const EXPLICIT_SECTIONS: Record<string, string> = {
  'legacy/workflow_app/tests/core-daily-02-contact.test.ts': 'app-crm',
  'legacy/workflow_app/tests/core-daily-07-08-recommendations.test.ts': 'app-crm',
}

export function sectionForFile(relPath: string): string | null {
  const normalized = relPath.replace(/^\.\//, '')
  const explicit = EXPLICIT_SECTIONS[normalized]
  if (explicit) return explicit
  for (const rule of SECTION_RULES) {
    if (rule.match.test(normalized)) return rule.section
  }
  return null
}

export function areaOf(sectionName: string): Area {
  return SECTIONS.find((s) => s.name === sectionName)?.area ?? 'APP'
}

/** Trees that can hold tests. `rust` and `scripts` exist; the rest are the pre-port trees, so a fence path
 * written before the port still resolves if that tree ever comes back. */
const TEST_TREES = ['rust', 'scripts', 'legacy/workflow_app/tests', 'testv2/engine_tests', 'agent-runtime']

/** Directories never walked: build output and dependencies are not tests. */
const SKIPPED_DIRS = new Set(['target', 'node_modules', 'dist'])

/**
 * Does this file HOLD tests?
 *
 * The TypeScript tree was one suite per file, so the suffix said everything. Rust puts the tests beside the
 * code they check, so the answer comes from the file's own content: an inline `#[cfg(test)]` module, or an
 * integration test under `tests/`. Finding them this way is what keeps the gate's one promise — a file
 * holding tests that no section runs is a file nobody will ever run again.
 */
function holdsTests(name: string, full: string): boolean {
  if (name.endsWith('.test.ts')) return true
  if (!name.endsWith('.rs')) return false
  if (full.endsWith('_test.rs') || full.includes('/tests/')) return true
  try {
    return readFileSync(full, 'utf8').includes('#[cfg(test)]')
  } catch {
    return false
  }
}

/** Every test file the runner knows about, relative to the repo root. */
export function listTestFiles(root: string): string[] {
  const found: string[] = []
  const walk = (dir: string): void => {
    for (const name of readdirSync(dir)) {
      if (SKIPPED_DIRS.has(name) || name.startsWith('.')) continue
      const full = join(dir, name)
      if (statSync(full).isDirectory()) walk(full)
      else if (holdsTests(name, full)) found.push(relative(root, full))
    }
  }
  for (const tree of TEST_TREES) {
    const full = join(root, tree)
    if (existsSync(full)) walk(full)
  }
  return found.sort()
}

export type SectionReport = {
  /** Section name -> its test files. */
  bySection: Record<string, string[]>
  /** Test files no rule claims — a file nobody will ever run. */
  unclassified: string[]
  total: number
}

/** Which sections the given changed paths touch. Area-level in V1, and it says so out loud. */
export function sectionsForPaths(root: string, paths: string[]): string[] {
  const report = sectionReport(root)
  const touched = new Set<string>()
  for (const path of paths) {
    const own = SECTIONS.map((s) => s.name).find((name) =>
      (report.bySection[name] ?? []).some((file) => file === path),
    )
    if (own) {
      touched.add(own)
      continue
    }
    // Prose and data change no tests. The honest answer is the empty list, not "everything", which is what a
    // person means when they say be careful.
    if (path.startsWith('docs/') || path.endsWith('.md')) continue
    // Root config can change how anything is built or run: the harness, plus the app core it builds.
    if (
      /^(package\.json|pnpm-lock\.yaml|tsconfig\.json|rust\/Cargo\.(toml|lock)|rust\/rust-toolchain|eslint)/.test(
        path,
      )
    ) {
      touched.add('harness')
      touched.add('app-core')
      continue
    }
    // THE PRE-PORT TREES. These are the only paths that answer with two sections, and the reason is in the
    // old fences: an engine change ran the engine's tests AND the verification rule set, which lived apart.
    if (path.startsWith('legacy/workflow_app/forge/')) {
      touched.add('forge-engine')
      touched.add('forge-verify')
      continue
    }
    if (path.startsWith('agent-runtime/')) {
      touched.add('forge-runtime')
      continue
    }
    // Everything else answers from THE SAME rules the report uses. It used to have a second hand-written
    // opinion here (`legacy/db/`, `lib/`, `app/`, `components/` → app-core), and the two drifted: a deleted
    // tree's branch kept answering app-core for paths the rules had stopped placing anywhere.
    const section = sectionForFile(path)
    if (section) touched.add(section)
  }
  return [...touched].sort()
}

export function sectionReport(root: string): SectionReport {

  const bySection: Record<string, string[]> = {}
  for (const section of SECTIONS) bySection[section.name] = []
  const unclassified: string[] = []
  const files = listTestFiles(root)
  for (const file of files) {
    const section = sectionForFile(file)
    if (!section || !bySection[section]) unclassified.push(file)
    else bySection[section]?.push(file)
  }
  return { bySection, unclassified, total: files.length }
}


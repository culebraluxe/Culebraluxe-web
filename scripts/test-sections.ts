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
import { existsSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

export type Area = 'FORGE' | 'APP' | 'HARNESS'

export type Section = {
  name: string
  area: Area
  /** What the section is for, in the words the runner prints. */
  about: string
}

export const SECTIONS: Section[] = [
  { name: 'forge-engine', area: 'FORGE', about: 'the engine: waves, dispatch, routing, contracts, gates' },
  { name: 'forge-verify', area: 'FORGE', about: 'verification itself: acceptance, assertions, receipts, migrations' },
  { name: 'forge-runtime', area: 'FORGE', about: 'the agent runtime: adapters, sessions, workspaces, invokers' },
  { name: 'app-crm', area: 'APP', about: 'the book of business: clients, deals, documents, deadlines' },
  { name: 'app-intake', area: 'APP', about: 'intake: mail, messages, Apple surfaces, calendar, conversations' },
  { name: 'app-money', area: 'APP', about: 'money: accounting, banking, statements' },
  { name: 'app-identity', area: 'APP', about: 'identity and access: auth, entitlements, people' },
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

/** Every test file the runner knows about, relative to the repo root. */
export function listTestFiles(root: string): string[] {
  const trees = ['legacy/workflow_app/tests', 'testv2/engine_tests', 'scripts', 'agent-runtime']
  const found: string[] = []
  const walk = (dir: string): void => {
    for (const name of readdirSync(dir)) {
      const full = join(dir, name)
      if (statSync(full).isDirectory()) walk(full)
      else if (name.endsWith('.test.ts')) found.push(relative(root, full))
    }
  }
  for (const tree of trees) {
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
    if (path.startsWith('scripts/')) touched.add('harness')
    // Root config can change how anything is built or run; the honest mapping is both cheap suites, not
    // "everything" (which is what a person means when they say be careful).
    else if (/^(package\.json|tsconfig\.json|next\.config\.|eslint)/.test(path)) {
      touched.add('harness')
      touched.add('app-core')
    } else if (path.startsWith('legacy/workflow_app/forge/')) {
      touched.add('forge-engine')
      touched.add('forge-verify')
    } else if (path.startsWith('agent-runtime/')) touched.add('forge-runtime')
    else if (
      path.startsWith('legacy/db/') ||
      path.startsWith('lib/') ||
      path.startsWith('app/') ||
      path.startsWith('components/')
    ) {
      touched.add('app-core')
    }
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


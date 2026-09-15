// ---------------------------------------------------------------------------
// forge:decision — the operator's interface to the decision institution (migration 180).
//
//   pnpm forge:decision list [--status active] [--format json]
//   pnpm forge:decision insert --key k --statement "one sentence" [--domain forge] [--role scout] [--apply]
//   pnpm forge:decision promote --key k --by captain [--evidence sha] [--apply]
//   pnpm forge:decision supersede --key k --by architect [--apply]
//   pnpm forge:decision mirror [--apply]      # write docs/agent/decisions/<key>.md
//   pnpm forge:decision check                 # files and rows agree (exit 1 when they do not)
//
// WHY A CLI AND NOT ONLY THE COCKPIT: promotion is a deliberate act by a person or the Architect, and
// the git mirror has to be written at the same moment - the packet asks for both so Grok can read the
// decisions without Neon access. A button that promoted without mirroring would create the split-brain
// the same packet warns about.
//
// DRY RUN BY DEFAULT, like `story:status`. `--apply` is the only thing that writes, so a typo in a key
// cannot promote a rule into force.
// ---------------------------------------------------------------------------

import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import {
  getDecision,
  insertDecisionCandidate,
  listDecisions,
  promoteDecision,
  supersedeDecision,
  DecisionError,
} from '@/db/forge-decision'
import {
  DECISION_DOMAINS,
  DECISION_SOURCES,
  DECISION_STATUSES,
  decisionMirrorPath,
  parseDecisionFile,
  renderDecisionFile,
  validateStatement,
  type ForgeDecision,
  type ForgeDecisionDomain,
  type ForgeDecisionRole,
  type ForgeDecisionSource,
  type ForgeDecisionStatus,
} from '@/lib/forge-decision'
import { describeControlPlane } from '@/lib/execution-target'
import { writeIfChanged } from '@/lib/artifact-file'

const MIRROR_DIR = 'docs/agent/decisions'

type Options = {
  command: string
  key: string
  statement: string
  domain: ForgeDecisionDomain
  source: ForgeDecisionSource
  status?: ForgeDecisionStatus
  role: ForgeDecisionRole
  evidence: string
  apply: boolean
  json: boolean
}

export function parseArgs(argv: string[]): Options {
  const options: Options = {
    command: argv[0] ?? 'list',
    key: '',
    statement: '',
    domain: 'forge',
    source: 'packet',
    role: 'captain',
    evidence: '',
    apply: false,
    json: false,
  }
  for (let i = 1; i < argv.length; i += 1) {
    const arg = argv[i]
    const value = argv[i + 1] ?? ''
    if (arg === '--key') options.key = value
    else if (arg === '--statement') options.statement = value
    else if (arg === '--domain') options.domain = value as ForgeDecisionDomain
    else if (arg === '--source') options.source = value as ForgeDecisionSource
    else if (arg === '--status') options.status = value as ForgeDecisionStatus
    else if (arg === '--role' || arg === '--by') options.role = value as ForgeDecisionRole
    else if (arg === '--evidence') options.evidence = value
    else if (arg === '--apply') options.apply = true
    else if (arg === '--format') options.json = value === 'json'
    else continue
    if (arg !== '--apply') i += 1
  }
  return options
}

/** Everything wrong with the request that can be known before any write. */
export function validateOptions(options: Options): string[] {
  const problems: string[] = []
  const needsKey = ['insert', 'promote', 'supersede'].includes(options.command)
  if (needsKey && !options.key) problems.push('--key is required')
  if (options.command === 'insert') problems.push(...validateStatement(options.statement))
  if (!(DECISION_DOMAINS as readonly string[]).includes(options.domain)) {
    problems.push(`--domain "${options.domain}" is not one of ${DECISION_DOMAINS.join(', ')}`)
  }
  if (!(DECISION_SOURCES as readonly string[]).includes(options.source)) {
    problems.push(`--source "${options.source}" is not one of ${DECISION_SOURCES.join(', ')}`)
  }
  if (options.status && !(DECISION_STATUSES as readonly string[]).includes(options.status)) {
    problems.push(`--status "${options.status}" is not one of ${DECISION_STATUSES.join(', ')}`)
  }
  return problems
}

function describe(decision: ForgeDecision): string {
  const when = decision.status === 'active' ? decision.promotedAt : decision.createdAt
  const owner = decision.owner ? ` · ${decision.owner}` : ''
  return `${decision.status.padEnd(10)} ${decision.key.padEnd(38)} ${decision.domain.padEnd(5)}${owner} · ${String(when).slice(0, 10)}`
}

/** Write the git mirror for one decision, and say whether it changed. */
export function mirrorDecision(root: string, decision: ForgeDecision): boolean {
  return writeIfChanged(join(root, decisionMirrorPath(decision.key)), renderDecisionFile(decision))
}

export type CheckFinding = { level: 'fail' | 'warn'; subject: string; message: string }

/**
 * THE SPLIT-BRAIN CHECK — do the files and the rows agree?
 *
 * The packet's rule 7, made mechanical: "a decision file without a matching active row (or a row without
 * a file after promote) is debt, not a silent split-brain". Candidates are exempt from having a file
 * (the mirror is written on promote), but a file that exists must match its row: a hand-edited status or
 * statement in `docs/agent/decisions/` is how a document starts reading as authority it does not have.
 */
export function checkMirror(root: string, decisions: readonly ForgeDecision[]): CheckFinding[] {
  const findings: CheckFinding[] = []
  const byKey = new Map(decisions.map((decision) => [decision.key, decision]))

  for (const decision of decisions) {
    if (decision.status === 'candidate') continue
    const path = decisionMirrorPath(decision.key)
    const full = join(root, path)
    if (!existsSync(full)) {
      findings.push({
        level: 'fail',
        subject: decision.key,
        message: `row is ${decision.status} but ${path} is missing — run: pnpm forge:decision mirror --apply`,
      })
      continue
    }
    const parsed = parseDecisionFile(readFileSync(full, 'utf8'), decision.key)
    if (parsed.key !== decision.key) {
      findings.push({ level: 'fail', subject: decision.key, message: `file says key "${parsed.key}"` })
    }
    if (parsed.status !== decision.status) {
      findings.push({
        level: 'fail',
        subject: decision.key,
        message: `file says status "${parsed.status}" but the row is "${decision.status}"`,
      })
    }
    if (parsed.statement !== decision.statement) {
      findings.push({ level: 'fail', subject: decision.key, message: 'file statement differs from the row' })
    }
  }

  const dir = join(root, MIRROR_DIR)
  for (const name of existsSync(dir) ? readdirSync(dir) : []) {
    if (!name.endsWith('.md')) continue
    const key = name.replace(/\.md$/, '')
    if (!byKey.has(key)) {
      findings.push({
        level: 'fail',
        subject: key,
        message: `docs/agent/decisions/${name} has no row in forge_decision`,
      })
    }
  }
  return findings
}

async function main(): Promise<number> {
  const root = process.cwd()
  const options = parseArgs(process.argv.slice(2))
  const problems = validateOptions(options)
  const plane = describeControlPlane()
  console.log(
    `forge:decision ${options.command} — target APP_ENV=${plane.appEnv} → ${plane.target}, ` +
      `${options.apply ? 'APPLY' : 'DRY RUN'}`,
  )
  if (problems.length > 0) {
    for (const problem of problems) console.error(`FAIL  ${problem}`)
    return 1
  }

  try {
    switch (options.command) {
      case 'list': {
        const decisions = await listDecisions({ status: options.status })
        if (options.json) {
          console.log(JSON.stringify({ count: decisions.length, decisions }, null, 2))
          return 0
        }
        if (decisions.length === 0) console.log('  (no decisions)')
        for (const decision of decisions) console.log(`  ${describe(decision)}`)
        console.log(`\n${decisions.length} decision(s)`)
        return 0
      }

      case 'insert': {
        const preview = {
          key: options.key,
          statement: options.statement,
          domain: options.domain,
          source: options.source,
        }
        if (!options.apply) {
          console.log(JSON.stringify({ wouldInsert: { ...preview, by: options.role, status: 'candidate' } }, null, 2))
          return 0
        }
        const created = await insertDecisionCandidate(preview, { role: options.role, name: options.role })
        console.log(`  inserted ${created.key} (${created.status}) — promote it to put it in force`)
        return 0
      }

      case 'promote': {
        if (!options.apply) {
          const existing = await getDecision(options.key)
          console.log(
            JSON.stringify(
              { wouldPromote: options.key, by: options.role, currentStatus: existing?.status ?? 'not found' },
              null,
              2,
            ),
          )
          return 0
        }
        const promoted = await promoteDecision(
          options.key,
          { role: options.role, name: options.role },
          { evidenceSha: options.evidence || null },
        )
        const changed = mirrorDecision(root, promoted)
        console.log(`  promoted ${promoted.key} by ${promoted.owner}`)
        console.log(`  mirror ${changed ? 'written' : 'unchanged'}: ${decisionMirrorPath(promoted.key)}`)
        return 0
      }

      case 'supersede': {
        if (!options.apply) {
          console.log(JSON.stringify({ wouldSupersede: options.key, by: options.role }, null, 2))
          return 0
        }
        const superseded = await supersedeDecision(options.key, { role: options.role, name: options.role })
        const changed = mirrorDecision(root, superseded)
        console.log(`  superseded ${superseded.key}`)
        console.log(`  mirror ${changed ? 'written' : 'unchanged'}: ${decisionMirrorPath(superseded.key)}`)
        return 0
      }

      case 'mirror': {
        const decisions = await listDecisions()
        let written = 0
        for (const decision of decisions) {
          // Candidates get a file only if one already exists: the mirror is written on promote, and
          // creating files for proposals is how the directory fills with rules that are not in force.
          if (decision.status === 'candidate' && !existsSync(join(root, decisionMirrorPath(decision.key)))) continue
          if (!options.apply) {
            console.log(`  would write ${decisionMirrorPath(decision.key)}`)
            continue
          }
          if (mirrorDecision(root, decision)) written += 1
        }
        console.log(`\n${options.apply ? `wrote ${written}` : 'dry run'} — ${decisions.length} row(s) considered`)
        return 0
      }

      case 'check': {
        const decisions = await listDecisions()
        const findings = checkMirror(root, decisions)
        const failures = findings.filter((finding) => finding.level === 'fail')
        if (options.json) {
          console.log(JSON.stringify({ decisions: decisions.length, failures: failures.length, findings }, null, 2))
          return failures.length > 0 ? 1 : 0
        }
        for (const finding of findings) {
          console.log(`${finding.level === 'fail' ? 'FAIL' : 'warn'}  ${finding.subject}\n      ${finding.message}`)
        }
        console.log(
          `\nforge:decision check — ${decisions.length} row(s), ${failures.length} failure(s)` +
            (failures.length > 0
              ? ' — rows and files disagree; that is debt, not a silent split-brain'
              : ', files and rows agree'),
        )
        return failures.length > 0 ? 1 : 0
      }

      default:
        console.error(`unknown command "${options.command}". Try: list | insert | promote | supersede | mirror | check`)
        return 1
    }
  } catch (error) {
    if (error instanceof DecisionError) {
      console.error(`FAIL  (${error.code}) ${error.message}`)
      return 1
    }
    throw error
  }
}

if (process.argv[1] && /(^|\/)forge-decision\.ts$/.test(process.argv[1])) {
  void main().then((code) => process.exit(code))
}

// ---------------------------------------------------------------------------
// FORGE DECISIONS — the store that outlives the agent that learned the lesson
// (ENG-FORGE-FACTORY-01 Phase 2, Object 2).
//
// WHY NOT MEMORY.md: memory is an incident narrative written for a human. A rule that must be
// true for every future lane cannot live in prose, because an agent reads it as one more document
// among many and a decision that only exists in a paragraph is not in force. Here each decision is
// ONE ROW with a stable key, and the lane runner injects the active ones into Lead/Smith context
// before they act.
//
// THE WRITE POLICY IS THE POINT OF THE MODULE. Three of the packet's rules are about who may do
// what, not about data: Scout inserts candidates only, Architect and the captain promote, Smith
// may not write at all. That is a question about ROLES, so it lives in one function used by the
// repository (`db/forge-decision.ts`) rather than being restated in every call site - and the
// refusal carries a reason, because "denied" with no reason teaches nobody anything.
//
// WHAT IS NOT HERE: the table, the seeds, and the git mirror's writer. The table and seeds are
// migration 180; the mirror is written by `scripts/forge-decision.ts` on promote.
// ---------------------------------------------------------------------------

export const DECISION_STATUSES = ['candidate', 'active', 'superseded'] as const
export type ForgeDecisionStatus = (typeof DECISION_STATUSES)[number]

export const DECISION_SOURCES = ['packet', 'hunter', 'incident', 'captain'] as const
export type ForgeDecisionSource = (typeof DECISION_SOURCES)[number]

export const DECISION_DOMAINS = ['forge', 'crm', 'web', 'ops'] as const
export type ForgeDecisionDomain = (typeof DECISION_DOMAINS)[number]

/** The roles the policy knows. `captain` is the human; `human` is an operator acting as one. */
export const DECISION_ROLES = [
  'scout',
  'architect',
  'lead',
  'smith',
  'inspector',
  'assay',
  'dev_ops',
  'captain',
  'human',
] as const
export type ForgeDecisionRole = (typeof DECISION_ROLES)[number]

export const DECISION_ACTIONS = ['insert-candidate', 'promote', 'supersede', 'flag-stale'] as const
export type ForgeDecisionAction = (typeof DECISION_ACTIONS)[number]

export type ForgeDecision = {
  id: string
  key: string
  statement: string
  status: ForgeDecisionStatus
  owner: string | null
  evidenceSha: string | null
  supersedesId: string | null
  source: ForgeDecisionSource
  domain: ForgeDecisionDomain
  createdAt: string
  promotedAt: string | null
  supersededAt: string | null
}

export type DecisionWriteVerdict = { allowed: boolean; reason: string }

/**
 * MAY THIS ROLE DO THIS TO A DECISION?
 *
 * Read from the packet's rules, and deliberately narrow:
 *   1. Scout may insert a candidate and nothing else — volume work proposes, it does not legislate.
 *   2. Architect or captain promote. Lead, Inspector, Assay and DEV_OPS may not, even though every
 *      one of them could technically write the row.
 *   3. Smith must not insert or promote. Not "should not": a decision written by the role that
 *      implements it is a rule the implementer chose for itself.
 *   4. Inspector may flag an ACTIVE decision as stale. That is the one write an inspector owns, and
 *      it deliberately cannot change the status: flagging is a report, superseding is a decision.
 */
export function decisionWritePolicy(role: ForgeDecisionRole, action: ForgeDecisionAction): DecisionWriteVerdict {
  const isHuman = role === 'captain' || role === 'human'
  if (isHuman) return { allowed: true, reason: 'the captain may write decisions directly' }

  switch (action) {
    case 'insert-candidate':
      if (role === 'scout') return { allowed: true, reason: 'Scout proposes candidates; promotion is not Scout’s to make' }
      if (role === 'architect') return { allowed: true, reason: 'Architect may propose as well as promote' }
      if (role === 'inspector') return { allowed: true, reason: 'Inspector may record a finding as a candidate' }
      if (role === 'smith') {
        return { allowed: false, reason: 'Smith must not write decisions: the role that implements a rule may not author it' }
      }
      return { allowed: false, reason: `role '${role}' reads decisions; it does not write them` }

    case 'promote':
      if (role === 'architect') return { allowed: true, reason: 'Architect promotes candidates to active' }
      if (role === 'scout') return { allowed: false, reason: 'Scout inserts candidates only; Architect or the captain promotes' }
      if (role === 'smith') return { allowed: false, reason: 'Smith must not promote: a rule promoted by its implementer is not a rule' }
      if (role === 'inspector') return { allowed: false, reason: 'Inspector may flag a stale decision; only Architect or the captain supersedes' }
      return { allowed: false, reason: `role '${role}' may not promote a decision` }

    case 'supersede':
      if (role === 'architect') return { allowed: true, reason: 'Architect supersedes an active decision' }
      return { allowed: false, reason: `only Architect or the captain supersedes; role '${role}' may not` }

    case 'flag-stale':
      if (role === 'inspector') return { allowed: true, reason: 'Inspector reports a decision as stale; the status is unchanged' }
      return { allowed: false, reason: `role '${role}' may not flag a decision as stale` }
  }
}

// ---------------------------------------------------------------------------
// Shape rules, mirrored from migration 180's CHECK constraints.
//
// The database refuses a two-sentence statement and so does this, for the same reason: the packet's
// first stop condition is "the decision table becomes a blog". Validating here means the failure is a
// sentence a writer can act on ("say it in one sentence") instead of a Postgres constraint name.
// ---------------------------------------------------------------------------

export const MAX_STATEMENT_LENGTH = 240

export function validateStatement(statement: string): string[] {
  const problems: string[] = []
  const trimmed = statement.trim()
  if (!trimmed) problems.push('a decision needs a statement')
  if (trimmed.length > MAX_STATEMENT_LENGTH) {
    problems.push(
      `statement is ${trimmed.length} characters; the cap is ${MAX_STATEMENT_LENGTH} (one sentence, not a paragraph)`,
    )
  }
  if (/[.!?]\s+\S/.test(trimmed)) {
    problems.push('statement reads as more than one sentence; say the one thing that is true')
  }
  return problems
}

/** Keys are slugs because they are referenced by humans, packets and mirror filenames. */
export function isValidDecisionKey(key: string): boolean {
  return /^[a-z0-9]+(-[a-z0-9]+)*$/.test(key) && key.length <= 80
}

/**
 * Which domain a story's decisions come from.
 *
 * The packet names four domains (`forge|crm|web|ops`) and the board names workstreams, so this is the
 * one mapping between them. Unknown input reads as `forge`: the engine's own work is the common case,
 * and a story that cannot be placed should still receive the factory invariants rather than nothing.
 */
export function decisionDomainForStory(input: {
  workstream?: string | null
  operatingSurface?: string | null
}): ForgeDecisionDomain {
  const token = `${input.workstream ?? ''} ${input.operatingSurface ?? ''}`.toUpperCase()
  if (token.includes('CRM') || token.includes('CONTACT') || token.includes('DEAL')) return 'crm'
  if (token.includes('WEB') || token.includes('SITE') || token.includes('MARKETING')) return 'web'
  if (token.includes('OPS') || token.includes('OPERAT') || token.includes('ACCOUNT')) return 'ops'
  return 'forge'
}

/**
 * Does this lane need the decision block?
 *
 * The packet: "Before Lead or Smith act, the runner injects the active decisions". `night` is Smith's
 * detached twin (same job, unattended), so it is included deliberately: an unattended lane that does
 * not know the rules is the exact lane that invents its own.
 */
export function laneNeedsDecisions(lane: string | null | undefined): boolean {
  return lane === 'lead' || lane === 'smith' || lane === 'night'
}

/** The cap the packet sets. Twenty statements is context; two hundred is a second handbook. */
export const DECISION_INJECTION_CAP = 20

export type DecisionSource = { key: string; statement: string; owner: string | null; promotedAt: string | null }

/**
 * The block a lane receives. BINDING, unlike the retrieved-material blocks: a decision is in force,
 * not a document to weigh. The closing line says where disagreement goes, because a rule with no
 * channel for disagreement gets ignored silently instead of challenged openly.
 */
export function renderDecisionBlock(decisions: readonly DecisionSource[]): string {
  if (decisions.length === 0) return ''
  const lines: string[] = []
  lines.push(`ACTIVE DECISIONS (${decisions.length}, newest promoted first) — these are IN FORCE for this domain.`)
  lines.push('They outlive this session and are not up for re-litigation by the lane that is acting on them.')
  for (const decision of decisions) {
    const owner = decision.owner ? ` (${decision.owner})` : ''
    lines.push(`- ${decision.key}: ${decision.statement}${owner}`)
  }
  lines.push(
    'If an active decision is wrong or stale, say so explicitly and open a learn item; do not act against it silently.',
  )
  return lines.join('\n')
}

/** `key` + mirror filename, kept together so the two can never disagree. */
export function decisionMirrorPath(key: string): string {
  return `docs/agent/decisions/${key}.md`
}

/**
 * The git mirror, so Grok (and any agent without Neon access) can read the decisions as files.
 *
 * The format is line-oriented and machine-parsed by `--check`, because a mirror nobody verifies
 * against the table is how a split-brain starts: the file drifts and then reads as authoritative.
 */
export function renderDecisionFile(decision: ForgeDecision): string {
  const lines: string[] = []
  lines.push(`# ${decision.key}`)
  lines.push('')
  lines.push('<!-- GENERATED from forge_decision. Do not hand-edit: pnpm forge:decision --mirror -->')
  lines.push('')
  lines.push(`- status: ${decision.status}`)
  lines.push(`- domain: ${decision.domain}`)
  lines.push(`- source: ${decision.source}`)
  lines.push(`- owner: ${decision.owner ?? ''}`)
  lines.push(`- evidence: ${decision.evidenceSha ?? ''}`)
  lines.push(`- promoted: ${decision.promotedAt ?? ''}`)
  lines.push(`- supersedes: ${decision.supersedesId ?? ''}`)
  lines.push('')
  lines.push(decision.statement)
  lines.push('')
  return lines.join('\n')
}

export type ParsedDecisionFile = {
  key: string
  status: string
  domain: string
  owner: string
  statement: string
}

/** The inverse of `renderDecisionFile`, strict enough to catch a hand edit. */
export function parseDecisionFile(markdown: string, fallbackKey = ''): ParsedDecisionFile {
  const lines = markdown.split('\n')
  const isField = (line: string) => /^-\s+([a-z_]+):\s*(.*)$/.test(line)
  const fields = new Map<string, string>()
  for (const line of lines) {
    const match = /^-\s+([a-z_]+):\s*(.*)$/.exec(line)
    if (match) fields.set(match[1], match[2].trim())
  }
  const title = lines.find((line) => line.startsWith('# '))?.slice(2).trim() ?? fallbackKey
  // The statement is the first real content line AFTER the last field, not after the first one: the
  // field block is a list, so slicing from the first match would read `- domain: forge` as the rule.
  const lastFieldIndex = lines.reduce((last, line, index) => (isField(line) ? index : last), -1)
  const statement =
    lines
      .slice(lastFieldIndex + 1)
      .map((line) => line.trim())
      .find((line) => line.length > 0 && !line.startsWith('<')) ?? ''
  return {
    key: title,
    status: fields.get('status') ?? '',
    domain: fields.get('domain') ?? '',
    owner: fields.get('owner') ?? '',
    statement,
  }
}


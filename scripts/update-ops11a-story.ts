// OPS-11A — record the Operational Issue Queue + Runbook dashboard story.
import { createStoryboardStory, updateStoryboardStory } from '../db/storyboard'

const id = 'OPS-11A'
const notes = `Deterministic Support/OPPS issue landing surface delivered. One durable table (issue): id, type, severity (RED/YELLOW/INFO), state (OPEN/RESOLVED), domain_type/domain_id, title, detail, detected_at, resolved_at — migration 081 with a partial unique index (uq_issue_open_once) as the duplicate-OPEN DB backstop (RESOLVED frees the slot so a returning condition creates fresh history). NO alert/escalation/subscriber/ACK framework; Attention untouched. Deterministic generation (db/issues.reconcileIssues) from canonical facts: MISSING_EXECUTED_PS (RED, under-contract deal with no signed agreement doc), APPRAISAL_OVERDUE (YELLOW, appraisal required + closing <=14d + no signed appraisal), CLOSING_DATE_AT_RISK (YELLOW, closing <=7d), OVERDUE_DEAL_TASK (YELLOW). Idempotent: INSERT where not already OPEN + resolve stale OPEN; verified live on DEV (reconcile twice -> no dupes). Bounded server-side read model (db/issues.getIssueQueue) paged 50, scope-filtered (OPERATIONS_EXCEPTION default; SUPPORT_EXCEPTION is an empty reusable sibling scope), sorted RED->YELLOW->INFO then oldest-first, with a separate COUNT total. Two-pane UI (components/portal/issues-queue.tsx) reuses Client surface grammar: left dominant dense queue (severity dot + title + typeLabel + related Deal/Property/Client + age), right resolution workspace (Issue Summary + Relevant Facts from canonical model + deterministic Runbook from lib/issue-types.ts code/config + Actions: Open Deal link + Mark Resolved via server action resolveIssueAction gated on crm.write). Empty state: 'No operational issues require attention.' DEV fixtures (scripts/seed-issue-fixtures.ts, production-guarded) demonstrate RED + YELLOW + RESOLVED across multiple domain objects. OPPS nav + registry entry added. Tests: workflow_app/tests/issues.test.ts 5/5; navigation-registry updated for the new OPS entry; live verify scripts/verify-issues.ts PASSED (14 checks: idempotency, dedupe, bounded/sorted queue, runbook, resolve leaves open queue + appears resolved + idempotent). tsc clean; next build passed; git diff --check clean. Completion is the human board decision after Chris visual QA.`

async function main() {
  const fields = {
    id,
    workstream: 'Operations',
    title: 'Operational Issue Queue + Runbook Dashboard',
    priority: 'High',
    status: 'In Progress',
    notes,
    batch: null,
    goal: 'Give Support/OPPS a deterministic landing surface showing what is currently wrong and what to do about it.',
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    operatingSurface: 'OPS',
    completion: 85,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
  }
  try {
    await createStoryboardStory(fields)
    console.log('created', id)
  } catch {
    await updateStoryboardStory(id, fields)
    console.log('updated', id)
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })

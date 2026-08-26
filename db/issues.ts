import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import {
  RUNBOOK,
  typesForResponsibility,
  issueTypeLabel,
} from '../lib/issue-types'
import type {
  IssueResponsibility,
  IssueRunbook,
  IssueSeverity,
  IssueState,
  IssueType,
} from '../lib/issue-types'

// ---------------------------------------------------------------------------
// OPS-11A — Operational Issue repository.
//
// ONE durable table (`issue`). This module owns:
//   - deterministic reconcile (create OPEN rows for real canonical conditions,
//     resolve OPEN rows whose condition has disappeared; idempotent + DB
//     deduped by the partial unique index uq_issue_open_once)
//   - a bounded, server-side queue read model (paged, scope-filtered, sorted
//     RED → YELLOW → INFO then oldest-first)
//   - the minimal resolve operation
// No alert/escalation/subscriber/ACK machinery. Attention is untouched.
// ---------------------------------------------------------------------------

// Deterministic desired-open conditions from canonical application/workflow
// facts. Reused (as a fragment) by both the create and resolve passes so they
// can never disagree.
const desiredCte = sql`
  with desired as (
    select src.type, src.severity, src.title, src.detail,
           src.domain_type, src.domain_id
    from (
      -- MISSING_EXECUTED_PS — under-contract deal with no fully-executed PS.
      select
        'MISSING_EXECUTED_PS'::text as type,
        'RED'::text as severity,
        'Purchase agreement not executed'::text as title,
        'Deal is under contract without a fully-executed purchase agreement on file.'::text as detail,
        'deal'::text as domain_type,
        d.id as domain_id
      from deal d
      where d.stage = 'under_contract'
        and not exists (
          select 1 from transaction_document t
          where t.deal_id = d.id and t.document_type = 'agreement' and t.state = 'signed'
        )
      union all
      -- APPRAISAL_OVERDUE — appraisal required, closing within 14 days, none signed.
      select
        'APPRAISAL_OVERDUE'::text as type,
        'YELLOW'::text as severity,
        'Appraisal overdue'::text as title,
        'Appraisal is required, closing is within 14 days, and no signed appraisal is on file.'::text as detail,
        'deal'::text as domain_type,
        d.id as domain_id
      from deal d
      where d.appraisal_required = true
        and d.stage = 'under_contract'
        and d.closing_date is not null
        and d.closing_date <= (current_date + 14)
        and not exists (
          select 1 from transaction_document t
          where t.deal_id = d.id and t.document_type = 'appraisal' and t.state = 'signed'
        )
      union all
      -- CLOSING_DATE_AT_RISK — under-contract deal closing within 7 days.
      select
        'CLOSING_DATE_AT_RISK'::text as type,
        'YELLOW'::text as severity,
        'Closing date at risk'::text as title,
        'Under-contract deal is scheduled to close within 7 days and has not closed.'::text as detail,
        'deal'::text as domain_type,
        d.id as domain_id
      from deal d
      where d.stage = 'under_contract'
        and d.closing_date is not null
        and d.closing_date <= (current_date + 7)
      union all
      -- OVERDUE_DEAL_TASK — open task on a deal past its due date.
      select
        'OVERDUE_DEAL_TASK'::text as type,
        'YELLOW'::text as severity,
        'Overdue deal task'::text as title,
        'An open task on this deal is past its due date.'::text as detail,
        'task'::text as domain_type,
        t.id as domain_id
      from task t
      where t.deal_id is not null
        and t.status = 'open'
        and t.due_at is not null
        and t.due_at < now()
    ) src
  )
`

export type ReconcileResult = {
  created: number
  resolved: number
  open: number
}

/**
 * Deterministic reconcile of OPEN issues against canonical facts. Idempotent:
 *   - creates an OPEN row for every desired condition absent an OPEN one
 *     (the partial unique index backstops duplicates)
 *   - resolves OPEN rows whose condition no longer holds
 * Re-running is safe and converges. If a condition returns later, a fresh OPEN
 * row is allowed because the resolved one freed the (type, domain) slot.
 */
export async function reconcileIssues(
  execute: QueryExecutor = sql,
): Promise<ReconcileResult> {
  const createdRows = (await execute`
    ${desiredCte}
    insert into issue (type, severity, state, domain_type, domain_id, title, detail)
    select d.type, d.severity, 'OPEN'::text, d.domain_type, d.domain_id, d.title, d.detail
    from desired d
    where not exists (
      select 1 from issue i
      where i.type = d.type
        and i.domain_type = d.domain_type
        and i.domain_id = d.domain_id
        and i.state = 'OPEN'
    )
    returning id
  `) as { id: string }[]

  const resolvedRows = (await execute`
    ${desiredCte}
    update issue i
    set state = 'RESOLVED', resolved_at = now()
    where i.state = 'OPEN'
      and not exists (
        select 1 from desired d
        where d.type = i.type
          and d.domain_type = i.domain_type
          and d.domain_id = i.domain_id
      )
    returning id
  `) as { id: string }[]

  const openRows = (await execute`
    select count(*)::int as open from issue where state = 'OPEN'
  `) as { open: number }[]

  return {
    created: createdRows.length,
    resolved: resolvedRows.length,
    open: Number(openRows[0]?.open ?? 0),
  }
}

// ---------------------------------------------------------------------------
// Bounded server-side queue read model.
// ---------------------------------------------------------------------------

export type IssueQueueRow = {
  id: string
  type: IssueType | string
  severity: IssueSeverity
  state: IssueState
  title: string
  detail: string | null
  domainType: string
  domainId: string
  detectedAt: string
  resolvedAt: string | null
  // bounded canonical resolution facts (never full objects)
  relatedDealId: string | null
  propertyName: string | null
  clientName: string | null
  closingDate: string | null
  dealStage: string | null
  taskTitle: string | null
  taskDueAt: string | null
  // derived
  typeLabel: string
  ageLabel: string
  runbook: IssueRunbook
}

export type IssuesPageResult = {
  rows: IssueQueueRow[]
  total: number
  page: number
  pageSize: number
  scope: IssueResponsibility
  state: IssueState
}

type IssueRaw = {
  id: string
  type: string
  severity: IssueSeverity
  state: IssueState
  title: string
  detail: string | null
  domain_type: string
  domain_id: string
  detected_at: string | null
  resolved_at: string | null
  related_deal_id: string | null
  property_name: string | null
  client_name: string | null
  closing_date: string | null
  deal_stage: string | null
  task_title: string | null
  task_due_at: string | null
  total: number
}

function formatAge(detectedAt: string | null): string {
  if (!detectedAt) return '—'
  const ms = Date.now() - new Date(detectedAt).getTime()
  if (Number.isNaN(ms) || ms < 0) return 'just now'
  const minutes = Math.floor(ms / 60000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h`
  const days = Math.floor(hours / 24)
  return days === 1 ? '1d' : `${days}d`
}

function genericRunbook(type: string): IssueRunbook {
  return {
    label: type,
    summary: 'No runbook guidance is configured for this issue type.',
    steps: [
      {
        title: 'Review canonical record',
        body: 'Open the related record and verify the underlying facts, then fix the canonical data or mark resolved once the condition is cleared.',
      },
    ],
  }
}

function mapRow(row: IssueRaw): IssueQueueRow {
  const type = row.type as IssueType
  const runbook = RUNBOOK[type] ?? genericRunbook(row.type)
  return {
    id: row.id,
    type: row.type,
    severity: row.severity,
    state: row.state,
    title: row.title,
    detail: row.detail ?? null,
    domainType: row.domain_type,
    domainId: row.domain_id,
    detectedAt: row.detected_at ?? '',
    resolvedAt: row.resolved_at ?? null,
    relatedDealId: row.related_deal_id ?? null,
    propertyName: row.property_name ?? null,
    clientName: row.client_name ?? null,
    closingDate: row.closing_date ?? null,
    dealStage: row.deal_stage ?? null,
    taskTitle: row.task_title ?? null,
    taskDueAt: row.task_due_at ?? null,
    typeLabel: runbook.label,
    ageLabel: formatAge(row.detected_at),
    runbook,
  }
}

/**
 * Server-side bounded queue. Returns only the requested page (50 default),
 * sorted RED → YELLOW → INFO then oldest-first, filtered to a responsibility
 * scope (default OPERATIONS_EXCEPTION) and state (default OPEN). A separate
 * COUNT(*) supplies the total for paging. Never loads every issue into the
 * browser.
 */
export async function getIssueQueue(
  opts: {
    scope?: IssueResponsibility
    state?: IssueState
    page?: number
    pageSize?: number
  } = {},
  execute: QueryExecutor = sql,
): Promise<IssuesPageResult> {
  const scope = opts.scope ?? 'OPERATIONS_EXCEPTION'
  const state = opts.state ?? 'OPEN'
  const page = Math.max(1, opts.page ?? 1)
  const pageSize = Math.max(1, Math.min(50, opts.pageSize ?? 50))
  const offset = (page - 1) * pageSize

  const types = typesForResponsibility(scope)
  if (types.length === 0) {
    return { rows: [], total: 0, page, pageSize, scope, state }
  }

  const rows = (await execute`
    with base as (
      select
        i.id, i.type, i.severity, i.state, i.title, i.detail,
        i.domain_type, i.domain_id, i.detected_at, i.resolved_at,
        case when i.domain_type = 'deal' then i.domain_id else t.deal_id end as related_deal_id,
        case when i.domain_type = 'task' then t.title else null end as task_title,
        case when i.domain_type = 'task' then t.due_at else null end as task_due_at
      from issue i
      left join task t on i.domain_type = 'task' and t.id = i.domain_id
      where i.state = ${state}
        and i.type = any(${types})
    ),
    joined as (
      select
        b.*, d.closing_date, d.stage as deal_stage,
        p.name as property_name, pe.display_name as client_name
      from base b
      left join deal d on d.id = b.related_deal_id
      left join property p on p.id = d.property_id
      left join person pe on pe.id = d.client_person_id
    )
    select j.*, count(*) over() as total
    from joined j
    order by
      case j.severity when 'RED' then 0 when 'YELLOW' then 1 else 2 end,
      j.detected_at asc,
      j.id asc
    limit ${pageSize} offset ${offset}
  `) as IssueRaw[]

  return {
    rows: rows.map(mapRow),
    total: Number(rows[0]?.total ?? 0),
    page,
    pageSize,
    scope,
    state,
  }
}

/** Minimal resolve operation — moves an OPEN issue to RESOLVED. Returns true
 *  when a row was transitioned (idempotent on already-resolved / missing). */
export async function resolveIssue(
  id: string,
  execute: QueryExecutor = sql,
): Promise<boolean> {
  const rows = (await execute`
    update issue
    set state = 'RESOLVED', resolved_at = now()
    where id = ${id} and state = 'OPEN'
    returning id
  `) as { id: string }[]
  return rows.length > 0
}

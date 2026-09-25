import { NextRequest, NextResponse } from 'next/server'

import {
  RUNBOOK,
  type IssueResponsibility,
  type IssueRunbook,
  type IssueState,
  type IssueType,
} from '@/lib/issue-types'
import { rustApiRead } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

const VALID_SCOPES: IssueResponsibility[] = [
  'OPERATIONS_EXCEPTION',
  'SUPPORT_EXCEPTION',
]
const VALID_STATES: IssueState[] = ['OPEN', 'RESOLVED']

type RustIssueRow = {
  id: string
  issueType: string
  severity: 'RED' | 'YELLOW' | 'INFO'
  state: IssueState
  title: string
  detail: string | null
  domainType: string
  domainId: string
  detectedAt: string
  resolvedAt: string | null
  relatedDealId: string | null
  propertyName: string | null
  clientName: string | null
  closingDate: string | null
  dealStage: string | null
  taskTitle: string | null
  taskDueAt: string | null
}

type RustIssuesPage = {
  rows: RustIssueRow[]
  total: number
  page: number
  pageSize: number
  scope: IssueResponsibility
  state: IssueState
}

function intParam(
  value: string | null,
  fallback: number,
  min: number,
  max: number,
) {
  const parsed = parseInt(value ?? '', 10)
  if (Number.isNaN(parsed)) return fallback
  return Math.max(min, Math.min(max, parsed))
}

function formatAge(detectedAt: string): string {
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

async function GETHandler(req: NextRequest) {
  const params = req.nextUrl.searchParams
  const scope = VALID_SCOPES.includes(
    params.get('scope') as IssueResponsibility,
  )
    ? (params.get('scope') as IssueResponsibility)
    : 'OPERATIONS_EXCEPTION'
  const state = VALID_STATES.includes(params.get('state') as IssueState)
    ? (params.get('state') as IssueState)
    : 'OPEN'
  const page = intParam(params.get('page'), 1, 1, Number.MAX_SAFE_INTEGER)
  const pageSize = intParam(params.get('pageSize'), 50, 1, 50)

  const result = await rustApiRead<RustIssuesPage>(
    (`/v1/issues?scope=${encodeURIComponent(scope)}&state=${encodeURIComponent(state)}&page=${page}&pageSize=${pageSize}`) as `/v1/${string}`,
  )

  return NextResponse.json({
    ...result.value,
    rows: result.value.rows.map((row) => {
      const runbook =
        RUNBOOK[row.issueType as IssueType] ?? genericRunbook(row.issueType)
      return {
        id: row.id,
        type: row.issueType,
        severity: row.severity,
        state: row.state,
        title: row.title,
        detail: row.detail,
        domainType: row.domainType,
        domainId: row.domainId,
        detectedAt: row.detectedAt,
        resolvedAt: row.resolvedAt,
        relatedDealId: row.relatedDealId,
        propertyName: row.propertyName,
        clientName: row.clientName,
        closingDate: row.closingDate,
        dealStage: row.dealStage,
        taskTitle: row.taskTitle,
        taskDueAt: row.taskDueAt,
        typeLabel: runbook.label,
        ageLabel: formatAge(row.detectedAt),
        runbook,
      }
    }),
  })
}

export const GET = withApiHandler(
  { label: '/api/portal/issues', route: '/api/portal/issues' },
  GETHandler,
)

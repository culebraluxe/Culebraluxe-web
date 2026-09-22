import { NextResponse, type NextRequest } from 'next/server'

import { getActivityFeed } from '@/legacy/db/activity-feed'
import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'
import { engineConfigured } from '@/legacy/workflow_app/engine-client'
import { getWorkflowDetail, getWorkflowSummaries } from '@/legacy/workflow_app/read-service'
import { resolveResponsibility } from '@/legacy/workflow_app/responsibility'
import { deadlineLabelFor } from '@/legacy/workflow_app/deadlines'

// A portal screen's payload in the screen's own shape — never flattened generic cells.
async function GETHandler(req: NextRequest): Promise<Response> {
  const actor = await getPortalActingUser()
  if (!actor) return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })

  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  switch (screen) {
    case 'activity': {
      const entries = await getActivityFeed(50)
      return NextResponse.json({
        activity: entries.map((entry) => ({
          id: entry.id,
          channel: entry.channel,
          direction: entry.direction,
          occurredAtLabel: entry.occurredAtLabel,
          title: entry.title,
          summary: entry.summary,
          personId: entry.personId,
          personName: entry.personName,
          propertyName: entry.propertyName,
          dealId: entry.dealId,
          dealPropertyName: entry.dealPropertyName,
        })),
      })
    }
    case 'workflows': {
      const configured = engineConfigured()
      const summaries = configured ? await getWorkflowSummaries() : []
      return NextResponse.json({
        workflows: {
          configured,
          items: summaries.map((summary) => ({
            instanceId: summary.instanceId,
            workflowName: summary.workflowName,
            workflowVersion: summary.workflowVersion,
            propertyName: summary.propertyName,
            status: summary.status,
            outcome: summary.outcome,
            activeMilestones: summary.activeMilestones,
            openTaskCount: summary.openTaskCount,
            blockerCount: summary.blockerCount,
            responsibleParty: summary.responsibleParty,
          })),
        },
      })
    }
    case 'workflow-record': {
      const scope = req.nextUrl.searchParams.get('scope')
      if (!scope) return NextResponse.json({ error: 'workflow-record requires scope.' }, { status: 400 })

      const detail = await getWorkflowDetail(scope)
      if (!detail) return NextResponse.json({ workflow: null })

      const completed = new Set(detail.completedNodes)
      const active = new Set(detail.currentNodes)
      const optional = new Set(detail.optionalNodes)
      const timelineIds = detail.displayOrder.length > 0 ? detail.displayOrder : detail.currentNodes

      return NextResponse.json({
        workflow: {
          instanceId: detail.instanceId,
          workflowName: detail.workflowName,
          workflowVersion: detail.workflowVersion,
          propertyName: detail.propertyName,
          status: detail.status,
          outcome: detail.outcome,
          responsibleParty: detail.responsibleParty,
          startedAtLabel: new Intl.DateTimeFormat('en-US').format(new Date(detail.startedAt)),
          timeline: timelineIds.map((id) => ({
            id,
            label: detail.nodeLabels[id] ?? id.replace(/_/g, ' '),
            description: detail.nodeDescriptions[id] ?? null,
            deadline: deadlineLabelFor(id) ?? null,
            completed: completed.has(id),
            active: active.has(id),
            optional: optional.has(id),
          })),
          milestones: detail.activeMilestoneNodeIds.map((id) => ({
            id,
            label: detail.nodeLabels[id] ?? id.replace(/_/g, ' '),
            owner: resolveResponsibility(detail.nodeResponsibility[id]).owner,
          })),
          openTaskCount: detail.openTaskCount,
          pendingTimerCount: detail.pendingTimerCount,
          blockers: detail.currentNodes
            .filter((id) => id.endsWith('_blocker'))
            .map((id) => detail.nodeLabels[id] ?? id),
          events: detail.events.map((event) => ({
            id: event.id,
            eventType: event.eventType,
            nodeLabel: event.nodeId ? (detail.nodeLabels[event.nodeId] ?? event.nodeId) : null,
            actor: event.actor,
          })),
        },
      })
    }
    default:
      return NextResponse.json({ error: `no portal payload source for screen '${screen}'` }, { status: 400 })
  }
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/page', route: '/api/portal/rust-ui/page' },
  GETHandler,
)

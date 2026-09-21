import { NextRequest, NextResponse } from 'next/server'

import { getExpenses, getReceivables, type Expense, type Receivable } from '@/legacy/db/accounting'
import { getActivityFeed } from '@/legacy/db/activity-feed'
import { listRecentErrors } from '@/legacy/db/app-error'
import { getAttentionSnapshot } from '@/legacy/db/attention'
import { getClientsPage, getClientById, type ClientSummary } from '@/legacy/db/clients'
import { getDashboardSnapshot } from '@/legacy/db/dashboard'
import { getDealWorkspace } from '@/legacy/db/deal-workspace'
import { getDeals } from '@/legacy/db/deals'
import { listForgeBatches, type ForgeBatch } from '@/legacy/db/forge-batch'
import { getIssueQueue } from '@/legacy/db/issues'
import { getMarketingContent } from '@/legacy/db/marketing-content'
import { getNeedsReviewItems, type NeedsReviewItem } from '@/legacy/db/needs-review'
import { getOpsCounts } from '@/legacy/db/ops-counts'
import { getPropertyAdmin, type PropertyAdminRow } from '@/legacy/db/property-admin'
import { getPropertyWorkspace } from '@/legacy/db/portal-property'
import { getShowings, type Showing } from '@/legacy/db/showings'
import { getStoryboardStory, listStoryboardStories, listStoryRuns, type StoryboardStory } from '@/legacy/db/storyboard'
import { getMarketingDashboard } from '@/legacy/db/syndication'
import { getSystemHealth } from '@/legacy/db/system-health'
import { listTraceEvents } from '@/legacy/db/workflow-trace'
import { getSecurityStatus } from '@/legacy/db/auth-status'
import { sql } from '@/legacy/db/client'
import { getClientAdmin } from '@/legacy/db/client-admin'
import { listFormInstances, getFormInstance } from '@/legacy/db/form-service-repository'
import { getIdentityQuality } from '@/legacy/db/identity-quality'
import { listPendingIntegrationInbox } from '@/legacy/db/integration-inbox'
import { getMediaAdmin } from '@/legacy/db/media-admin'
import { getPropertyMediaCoverage } from '@/legacy/db/property-media-coverage'
import { getPnlStatement, getAccountingDashboard } from '@/legacy/db/accounting'
import { getClients } from '@/legacy/db/clients'
import { listSprintRollups } from '@/legacy/db/sprint'
import { listAgentWorkItems } from '@/legacy/db/agent-work'
import { getReportingSnapshot } from '@/legacy/db/reporting'
import { getSettingsAuthorities, getSettingsRoles, getSettingsUsers } from '@/legacy/db/settings-auth'
import { getFactoryCommandCenterSnapshot } from '@/lib/factory-command-center-data'
import { listIssuedDocuments } from '@/lib/vault-io'
import { engineConfigured } from '@/legacy/workflow_app/engine-client'
import { getWorkflowSummaries } from '@/legacy/workflow_app/read-service'
import { SqlProjectRepository } from '@/legacy/db/project-service-repository'
import { AuthError } from '@/lib/auth/errors'
import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'
import type { Deal } from '@/lib/portal/types'

// ---------------------------------------------------------------------------
// ROWS FOR THE RUST UI.
//
// The browser never calls the Rust API and the WASM module never makes a request: it renders a model and asks for
// data through a DOM event, the TypeScript host fetches from *this* route, and the answer goes back into the model
// through `rows_loaded`. That is why this route exists in the application rather than in Rust — the session, the
// credentials and the permission checks are all here, where they already were.
//
// Shape (`Row` in rust/ui/src/model.rs, camelCase on both sides):
//   { id: string, cells: string[], badge?: string | null }
//
// The `cells` array is deliberately generic. This pass ports screen *structure* — route, heading, navigation, state
// boundary — so only the screens whose real columns have actually been read get real rows here. Everything else
// answers `[]` and the screen says "Nothing to show yet", which is honest: an invented column is a lie the next
// reader has to disprove.
//
// Wired: every portal menu screen except `accounting-receipt-scanner` (no read model found). Each loader calls the
// same repository or read model the live TypeScript screen calls, so the two cannot disagree about the data; the
// mapping from DTO to row is a projection and nothing more.
//
// AUTHORITY: authenticated portal users only. A per-screen authority check (who may read expenses, who may read
// flight recorder) has to be decided per screen and is NOT yet applied here — so this route stays read-only, and any
// screen with a narrower audience must not be added to the list below until its authority is chosen.
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

type RustUiRow = { id: string; cells: string[]; badge?: string | null }

/** Amounts are numbers in the read models and money on the screen. */
const money = (amount: number) =>
  new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 }).format(amount)

/**
 * One named fact on a record screen. `null` means the fact is absent, so the caller drops it: a record page that
 * prints eleven dashes to show five facts reads worse than one that prints five, and "unknown" and "empty" are not
 * the same thing to a reader.
 */
const fact = (label: string, value: string | number | null | undefined): RustUiRow | null => {
  if (value === null || value === undefined || value === '') return null
  return { id: label, cells: [label, String(value)] }
}

const facts = (rows: (RustUiRow | null)[]): RustUiRow[] => rows.filter((row): row is RustUiRow => row !== null)

/**
 * Field names are the read model's own, so a screen can be wired before its DTO has been read line by line. Nothing
 * here invents a column: the label IS the field name and the value IS the value, and where a screen's real columns are
 * known it uses a mapper of its own instead.
 */
const label = (key: string): string =>
  key
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/[_-]/g, ' ')
    .replace(/^./, (character) => character.toUpperCase())

function primitiveCells(item: unknown): string[] {
  if (item === null || item === undefined) return []
  if (typeof item !== 'object') return [String(item)]
  return Object.values(item as Record<string, unknown>).flatMap((entry) => {
    if (entry === null || entry === undefined) return []
    if (typeof entry === 'object') return [Array.isArray(entry) ? `${entry.length}` : '']
    return [String(entry)]
  })
}

function factRowsFrom(value: unknown): RustUiRow[] {
  if (value === null || value === undefined) return []
  if (Array.isArray(value)) {
    // A list read model: one row per item.
    return value.flatMap((item, index) => {
      const cells = primitiveCells(item)
      return cells.length ? [{ id: `item-${index}`, cells }] : []
    })
  }
  return Object.entries(value as Record<string, unknown>)
    .filter(
      ([, entry]) =>
        entry === null || Array.isArray(entry) || ['string', 'number', 'boolean'].includes(typeof entry),
    )
    .map(([key, entry]) => ({
      id: key,
      cells: [label(key), Array.isArray(entry) ? `${entry.length}` : String(entry ?? '—')],
      badge: Array.isArray(entry) ? 'count' : undefined,
    }))
}

function activityRows(
  entries: Awaited<ReturnType<typeof getActivityFeed>>,
): RustUiRow[] {
  return entries.map((entry) => ({
    id: entry.id,
    cells: [
      entry.title ?? '(no title)',
      entry.occurredAtLabel,
      entry.personName ?? entry.propertyName ?? entry.dealPropertyName ?? '—',
    ],
    badge: entry.channel,
  }))
}

/**
 * Columns taken from `ClientSummary`, not invented: the row shows what the directory already resolves — a name that
 * says whether it was resolved (`nameResolved`), the role, where they are, who owns them, and when they were last
 * contacted. The read model does the work; this is a projection, and it stays one.
 */
function clientRows(client: ClientSummary): RustUiRow {
  return {
    id: client.id,
    cells: [
      client.nameResolved ? client.displayName : `${client.displayName} (unresolved)`,
      client.role,
      client.location ?? '—',
      client.assignedAgent ?? '—',
      client.lastContactLabel ?? 'No contact yet',
    ],
    badge: client.status,
  }
}

function dealRows(deal: Deal): RustUiRow {
  return {
    id: deal.id,
    cells: [
      deal.propertyName,
      deal.clientName,
      deal.listPrice ? money(deal.listPrice) : '—',
      deal.nextMilestone ?? deal.nextMilestoneAt ?? '—',
      deal.owner,
    ],
    // The stage is the badge because it is the axis the live Deals board groups by.
    badge: deal.stage,
  }
}

function showingRows(showing: Showing): RustUiRow {
  return {
    id: showing.id,
    cells: [
      showing.personName,
      showing.propertyName ?? showing.dealPropertyName ?? '—',
      showing.scheduledAtLabel ?? showing.requestedAtLabel,
      showing.feedback ?? '—',
    ],
    badge: showing.status,
  }
}

function propertyRows(property: PropertyAdminRow): RustUiRow {
  return {
    id: property.id,
    cells: [
      property.name,
      property.location ?? '—',
      property.listPrice ? money(property.listPrice) : 'No price',
      `${property.bedrooms ?? '—'} bd / ${property.bathrooms ?? '—'} ba`,
      property.slug ?? 'No slug',
      property.issueCount === 0 ? 'No issues' : `${property.issueCount} issue(s)`,
    ],
    badge: property.status,
  }
}

function needsReviewRows(item: NeedsReviewItem): RustUiRow {
  return {
    id: item.id,
    cells: [
      item.displayName,
      item.requestType,
      item.propertyName ?? '—',
      item.receivedAtLabel,
      item.message?.slice(0, 120) ?? '—',
    ],
    badge: item.status,
  }
}

function expenseRows(expense: Expense): RustUiRow {
  return {
    id: expense.id,
    cells: [expense.vendor, expense.category, money(expense.amount), expense.expenseOn],
    badge: expense.status,
  }
}

function receivableRows(receivable: Receivable): RustUiRow {
  return {
    id: receivable.id,
    cells: [
      receivable.description,
      money(receivable.amount),
      receivable.dueOn ?? '—',
      receivable.personName ?? receivable.dealName ?? receivable.propertyName ?? '—',
    ],
    badge: receivable.status,
  }
}

function storyboardRows(story: StoryboardStory): RustUiRow {
  return {
    id: story.id,
    cells: [
      story.title,
      story.workstream,
      story.priority,
      story.batch === null ? 'no batch' : `batch ${story.batch}`,
      story.operatingSurface ?? 'unclassified',
    ],
    badge: story.status,
  }
}

/** Dashboard tasks and recent interactions — the whole snapshot the live screen renders. */
function dashboardRows(snapshot: Awaited<ReturnType<typeof getDashboardSnapshot>>): RustUiRow[] {
  const overdue = snapshot.overdueTasks.map((task) => ({
    id: task.id,
    cells: [task.title, task.dueAtLabel ?? '—', task.contextName ?? '—'],
    badge: 'overdue',
  }))
  const dueSoon = snapshot.tasksDueSoon.map((task) => ({
    id: `soon-${task.id}`,
    cells: [task.title, task.dueAtLabel ?? '—', task.contextName ?? '—'],
    badge: 'due soon',
  }))
  const recent = snapshot.recentInteractions.map((interaction) => ({
    id: `recent-${interaction.id}`,
    cells: [
      interaction.title ?? interaction.summary ?? '(interaction)',
      interaction.occurredAtLabel,
      interaction.personName,
    ],
    badge: interaction.channel,
  }))
  return [...overdue, ...dueSoon, ...recent]
}

function attentionRows(snapshot: Awaited<ReturnType<typeof getAttentionSnapshot>>): RustUiRow[] {
  const tasks = (badge: string, items: typeof snapshot.overdueTasks) =>
    items.map((task) => ({
      id: `${badge.replace(' ', '-')}-${task.id}`,
      cells: [
        task.title,
        task.dueAtLabel ?? '—',
        task.personName ?? task.propertyName ?? task.dealPropertyName ?? '—',
      ],
      badge,
    }))
  // The quiet-but-important list is half of what this screen is for: a relationship with no open work and no recent
  // contact is exactly the one that gets forgotten, so it comes from the same field the live page uses.
  const quiet = snapshot.quietButImportant.map((person) => ({
    id: `quiet-${person.id}`,
    cells: [
      person.displayName,
      `${person.role} · ${person.activeDealCount} active deal(s)`,
      person.lastContactLabel ?? 'never contacted',
    ],
    badge: 'quiet',
  }))
  const busy = snapshot.peopleWithOpenWork.map((person) => ({
    id: `busy-${person.id}`,
    cells: [
      person.displayName,
      `${person.role} · ${person.openTaskCount} open task(s)`,
      person.lastContactLabel ?? '—',
    ],
    badge: 'open work',
  }))
  return [...tasks('overdue', snapshot.overdueTasks), ...tasks('due soon', snapshot.dueSoonTasks), ...quiet, ...busy]
}

async function errorRows(limit = 25): Promise<RustUiRow[]> {
  const rows = await listRecentErrors(limit)
  return rows.map((row) => ({
    id: row.id,
    // `message` and `code` are both nullable, and an error row with neither is still a real row: the kind is the
    // minimum. Saying "(no message)" beats an empty cell.
    cells: [row.kind, row.operation ?? row.route ?? '—', row.message ?? row.code ?? '(no message)'],
    badge: row.level,
  }))
}

async function traceRows(
  filter: { limit?: number; workflowInstanceId?: string } = { limit: 50 },
): Promise<RustUiRow[]> {
  const rows = await listTraceEvents(filter)
  return rows.map((row) => ({
    // A trace row's id is nullable by design (the durable key is assigned by the writer), so the position in the list
    // is the only stable handle. A synthetic id beats a row that cannot be selected.
    id: row.id ?? `${row.eventType}-${row.occurredAt}-${row.system}`,
    cells: [row.eventType, row.system, row.occurredAt, row.outcome ?? '—'],
    badge: row.durationMs === null ? undefined : `${row.durationMs}ms`,
  }))
}

async function clientRecordRows(id: string): Promise<RustUiRow[]> {
  // `getClientById` is the full canonical client ("for the working-pane detail"), which is exactly what a record
  // screen is. Its fields are NOT the directory summary's: `ClientSummary` has `primaryEmail`/`lastContactLabel`
  // because it is built for a list, while this one has `email`/`phone` and a `lastContact` object.
  const client = await getClientById(id)
  // An id that does not resolve is an empty screen, not an error: the row that opened this could have been from a
  // stale list, and "no such client" is a fact rather than a failure.
  if (!client) return []
  const activity = client.relationshipActivity
  const budget =
    client.budgetMin === null || client.budgetMin === undefined || client.budgetMax === null || client.budgetMax === undefined
      ? null
      : `${money(client.budgetMin)} – ${money(client.budgetMax)}`
  return facts([
    fact('Name', client.displayName),
    fact('Role', client.role),
    fact('Status', client.status),
    fact('Location', client.location),
    fact('Email', client.email),
    fact('Phone', client.phone),
    fact('Assigned agent', client.assignedAgent),
    fact('Budget', budget),
    fact('Preferred areas', client.preferredAreas?.length ? client.preferredAreas.join(', ') : null),
    fact('Property types', client.propertyTypes?.length ? client.propertyTypes.join(', ') : null),
    fact('Priorities', client.priorities?.length ? client.priorities.join(', ') : null),
    fact('Timeline', client.timeline),
    fact('Interactions', client.interactions.length ? `${client.interactions.length}` : null),
    fact('Property interests', client.propertyInterests.length ? `${client.propertyInterests.length}` : null),
    fact(
      'Last contact',
      client.lastContact
        ? `${client.lastContact.channel} on ${client.lastContact.occurredAt}${
            client.lastContact.summary ? ` — ${client.lastContact.summary}` : ''
          }`
        : null,
    ),
    fact(
      'Next action',
      client.nextAction ? `${client.nextAction.title} (${client.nextAction.occurredAt})` : null,
    ),
    fact('Notes', client.notes),
    // The relationship activity is optional on this type, so every read of it is guarded: an absent one means the
    // evidence seam did not run, which is different from "no evidence found" and is not printed as if it were.
    fact(
      'Evidence',
      activity
        ? activity.hasEvidence
          ? `${activity.observedCommunicationCount} observed communication(s) from ${activity.sources.join(', ')}`
          : 'none'
        : null,
    ),
    fact('Two-way', activity ? (activity.twoWay ? 'yes' : 'no') : null),
    fact('Coverage limited', activity ? (activity.coverageLimited ? 'yes' : 'no') : null),
  ])
}

async function dealRecordRows(id: string, actor: ActingActor): Promise<RustUiRow[]> {
  const workspace = await getDealWorkspace(id, actor)
  const { deal, property, client } = workspace
  return facts([
    fact('Stage', deal?.stage),
    fact('List price', deal?.listPrice ? money(deal.listPrice) : null),
    fact('Offer price', deal?.offerPrice ? money(deal.offerPrice) : null),
    fact('Closing date', deal?.closingDateLabel),
    fact('Closed', deal?.closedAtLabel),
    fact('Created', deal?.createdAtLabel),
    fact('Updated', deal?.updatedAtLabel),
    fact('Notes', deal?.notes),
    fact('Property', property?.name),
    fact('Location', property?.location),
    fact('Type', property?.propertyType),
    fact('Bedrooms', property?.bedrooms),
    fact('Bathrooms', property?.bathrooms),
    fact('Square feet', property?.squareFeet),
    fact('Client', client?.displayName),
    fact('Client role', client?.role),
    fact('Client status', client?.status),
    fact('Client email', client?.email),
  ])
}

/**
 * A property record, as counts and presence rather than the fact table: `PropertyFactsView`'s field names are not
 * something this route has verified, and a record page that renders wrong labels is worse than one that renders
 * fewer. The related sets are the same ones the workspace read model builds for the live screen.
 */
async function propertyRecordRows(id: string): Promise<RustUiRow[]> {
  const workspace = await getPropertyWorkspace(id)
  const count = (label: string, items: unknown[]) => fact(label, items.length === 0 ? null : `${items.length}`)
  return facts([
    fact('Seller', workspace.seller ? 'on record' : null),
    count('Media items', workspace.media),
    count('Open tasks', workspace.openTasks),
    count('Recent activity', workspace.activity),
    count('Interested clients', workspace.interests),
    count('Enquiries', workspace.enquiries),
    count('Showings', workspace.showings),
    count('Deals', workspace.deals),
  ])
}

async function storyRecordRows(id: string): Promise<RustUiRow[]> {
  const story = await getStoryboardStory(id)
  if (!story) return []
  // The registry says the Flight Recorder is reached from the SELECTED STORY's own detail pane, "for the instance that
  // actually ran it". I looked for that link and it is NOT in this read model: `legacy/db/storyboard.ts` has no workflow
  // instance id on StoryRun (id, storyId, startedAt, endedAt, resultStatus, runType, agentRuntime, completion, …) and
  // `listTraceEvents` filters by workflowInstanceId, dealId, traceId or correlationId — none of which is a run.
  // So the runs are SHOWN here and no row pretends to open a trace. Finding, not a guess.
  const runs = await listStoryRuns(id)
  return facts([
    fact('Title', story.title),
    fact('Workstream', story.workstream),
    fact('Status', story.status),
    fact('Priority', story.priority),
    fact('Operating surface', story.operatingSurface ?? 'unclassified'),
    fact('Batch', story.batch === null ? null : `batch ${story.batch}`),
    fact('Deploy deferred', story.batchDeploy ? 'yes' : 'no'),
    fact('Goal', story.goal),
    fact('Scope', story.scope),
    fact('Dependencies', story.dependencies),
    fact('Preconditions', story.preconditions),
    fact('Acceptance criteria', story.acceptanceCriteria),
    fact('Notes', story.notes),
    ...(runs ?? []).map((run) => ({
      // A run id, not an instance id: there is no instance id in this read model, so this row selects and does not
      // pretend to open anything.
      id: `run-${run.id}`,
      cells: [
        'Run',
        [run.runType, run.resultStatus, run.agentRuntime, run.startedAt, run.endedAt ? `→ ${run.endedAt}` : null]
          .filter(Boolean)
          .join(' · '),
      ],
      badge: run.resultStatus ?? undefined,
    })),
  ])
}

function batchRows(batch: ForgeBatch): RustUiRow {
  return {
    id: batch.id,
    cells: [
      batch.label ?? `batch ${batch.id.slice(0, 8)}`,
      `${batch.storyCount} story/stories`,
      `${batch.queuedCount} queued · ${batch.skippedCount} skipped`,
      String(batch.createdAt).slice(0, 10),
    ],
    badge: batch.status,
  }
}

/** `activePropertyCount` -> `Active property count`, so a data-quality row can be read by a human. */
function humanise(key: string): string {
  const words = key.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase()
  return words.charAt(0).toUpperCase() + words.slice(1)
}

/**
 * The tech screen's real content is its invariants: every field of the health snapshot is a count that should be zero
 * or a count that should be understood. Showing only the non-zero ones means the screen answers "is anything wrong"
 * with rows instead of reassurance — and an all-clear snapshot says so in one line rather than showing thirty zeros.
 */
function healthRows(snapshot: Awaited<ReturnType<typeof getSystemHealth>>): RustUiRow[] {
  const rows = (Object.entries(snapshot) as [string, unknown][])
    .filter(([, value]) => typeof value === 'number' && value > 0)
    .map(([key, value]) => ({
      id: key,
      cells: [humanise(key), String(value)],
      badge: /without|missing|notImage|mismatch/i.test(key) ? 'attention' : 'count',
    }))
  return rows.length === 0
    ? [{ id: 'all-clear', cells: ['Every health invariant reads zero', ''], badge: 'ok' }]
    : rows
}

/** Counts are not a list, but they are the whole content of these screens: one row per number, labelled. */
function countRows(counts: Record<string, number>, badge: string): RustUiRow[] {
  return Object.entries(counts).map(([key, value]) => ({
    id: key,
    cells: [humanise(key), String(value)],
    badge,
  }))
}

function marketingRows(blocks: Awaited<ReturnType<typeof getMarketingContent>>): RustUiRow[] {
  // The repository returns a Result rather than throwing, so an unavailable database is a sentence the operator can
  // read instead of a 500 with no explanation.
  if (!blocks.ok) {
    throw new Error(`marketing content is unavailable: ${blocks.error.kind}`)
  }
  return blocks.data.map((block) => ({
    id: block.id,
    cells: [block.title ?? '(untitled)', block.subtitle ?? block.eyebrow ?? '—', block.ctaLabel ?? '—'],
    badge: block.kind,
  }))
}

/** The issue queue, shared by the Issue Queue screen and the retired Command Console. */
async function issueQueueRows(): Promise<RustUiRow[]> {
  const page = await getIssueQueue({ pageSize: 50 })
  return page.rows.map((row) => ({
    id: row.id,
    cells: [row.title, row.type, row.severity, row.domainType],
    badge: row.state,
  }))
}

/**
 * The WhatsApp diagnostic, mirroring `app/portal/admin/whatsapp-meta/page.tsx`: it asks Meta for the WABA's phone
 * numbers and reports what comes back, including the failure verbatim. This is the screen used to prove the integration
 * works, so a failed call must read as the error Meta returned rather than as an empty list.
 */
async function metaWhatsappRows(): Promise<RustUiRow[]> {
  const wabaId = process.env.WHATSAPP_WABA_ID?.trim() || '1605543247626812'
  const token = process.env.WHATSAPP_ACCESS_TOKEN?.trim()
  const rows: RustUiRow[] = [
    { id: 'waba', cells: ['WABA id', wabaId] },
    { id: 'token', cells: ['Access token', token ? 'configured' : 'NOT configured'], badge: token ? 'ok' : 'missing' },
    { id: 'verify', cells: ['Verify token', process.env.WHATSAPP_VERIFY_TOKEN ? 'configured' : 'NOT configured'] },
  ]
  if (!token) return rows

  try {
    const response = await fetch(
      `https://graph.facebook.com/v23.0/${wabaId}/phone_numbers?access_token=${encodeURIComponent(token)}`,
      { cache: 'no-store' },
    )
    const payload = (await response.json()) as {
      data?: { id?: string; display_phone_number?: string; verified_name?: string; quality_rating?: string; code_verification_status?: string }[]
      error?: { message?: string; type?: string; code?: number }
    }
    if (payload.error) {
      return [...rows, { id: 'meta-error', cells: ['Meta error', payload.error.message ?? 'unknown'], badge: payload.error.code?.toString() }]
    }
    for (const phone of payload.data ?? []) {
      rows.push({
        id: phone.id ?? phone.display_phone_number ?? 'phone',
        cells: [
          phone.display_phone_number ?? '(no number)',
          phone.verified_name ?? '—',
          phone.code_verification_status ?? '—',
        ],
        badge: phone.quality_rating,
      })
    }
    if ((payload.data ?? []).length === 0) {
      rows.push({ id: 'no-phones', cells: ['No phone numbers returned for this WABA', ''], badge: 'empty' })
    }
    return rows
  } catch (cause) {
    return [...rows, { id: 'fetch-error', cells: ['Could not reach Meta', cause instanceof Error ? cause.message : String(cause)], badge: 'error' }]
  }
}

/** The coexistence configuration the embedded signup needs, as facts — what you check before testing a signup. */
function coexistenceRows(): RustUiRow[] {
  return [
    { id: 'app', cells: ['Meta app id', process.env.META_APP_ID ?? '1573618894304413'] },
    { id: 'config', cells: ['Configuration id', process.env.META_CONFIGURATION_ID ?? '1416075310402629'] },
    { id: 'graph', cells: ['Graph version', 'v26.0'] },
    { id: 'app-secret', cells: ['App secret', process.env.META_APP_SECRET ? 'configured' : 'NOT configured'] },
    { id: 'verify', cells: ['Verify token', process.env.WHATSAPP_VERIFY_TOKEN ? 'configured' : 'NOT configured'] },
    { id: 'access', cells: ['Access token', process.env.WHATSAPP_ACCESS_TOKEN ? 'configured' : 'NOT configured'] },
  ]
}

/**
 * Design Lab is a live gallery of the portal's own primitives — it renders the real components, so what a rows endpoint
 * can carry is what the lab CONTAINS, not the components themselves. The live rendering stays TypeScript until the port
 * can render a screen of its own markup rather than rows.
 */
function designLabRows(): RustUiRow[] {
  const primitives: [string, string][] = [
    ['PageHeader', 'title, eyebrow, actions'],
    ['Panel', 'the surface container'],
    ['PortalCombobox', 'single select with search'],
    ['PortalDialog / PortalDialogClose', 'modal with a close affordance'],
    ['PortalFieldset / PortalLegend', 'grouped fields with a legend'],
    ['PortalField / PortalFieldLabel', 'label and control pairing'],
    ['PortalFieldDescription / PortalFieldError', 'help text and error state'],
    ['PortalInput / PortalTextarea', 'text entry'],
    ['PortalSelect', 'option list'],
  ]
  const shown: [string, string][] = [
    ['Compact filter row', 'search + stage filter + reset'],
    ['Compact segmented control', 'all stages / active / showing / offer / under contract / blocked'],
    ['Field contract', 'the label + control + description + error shape'],
    ['Surface rail (as in the shell)', 'the operating-surface rail'],
    ['Table', 'deal rows with stage, value, next date'],
  ]
  return [
    ...primitives.map(([name, what]) => ({ id: `primitive-${name}`, cells: [name, what], badge: 'primitive' })),
    ...shown.map(([name, what]) => ({ id: `demo-${name}`, cells: [name, what], badge: 'demo' })),
  ]
}

/**
 * Projects, through the same repository the live screen builds. The screen's three widgets (tree, Gantt, calendar) stay
 * in TypeScript; what the port can carry is the project list itself, which is the data those widgets are drawn over.
 */
async function projectRows(): Promise<RustUiRow[]> {
  const projects = await new SqlProjectRepository(sql).list()
  // The live work plan table's columns: Work item, Owner, Status. Mirrored rather than chosen.
  return projects.map((project) => ({
    id: project.id,
    cells: [project.name ?? project.id, project.owner ?? '—'],
    badge: project.status,
  }))
}

type ActingActor = Awaited<ReturnType<typeof getPortalActingUser>>

/**
 * Which screens have real rows, and where those rows come from. A map rather than a switch so that "what is wired" is
 * one glance, and so adding a screen is one line.
 *
 * NOT here, on purpose:
 *   - `accounting-receipt-scanner`: no read model found for it. Answering [] is honest; inventing a shape is not.
 *   - `projects`: the Rust side never asks for it (a deferred screen loads nothing), so a loader would be dead code.
 *   - `form-record` and the other id-keyed pages from the live tree (`workflows/[instanceId]`,
 *     `runtime-inspector/[instanceId]`, `command-console/[storyId]`): their read models are not verified yet. The
 *     screen variants do not exist either, so nothing asks for them.
 */
type ScreenLoader = (actor: ActingActor, scope: string | null) => Promise<RustUiRow[]>

/** A record screen with no scope cannot be fetched, and saying so beats fetching the wrong thing. */
const requireScope = (scope: string | null, screen: string): string => {
  if (!scope) throw new Error(`${screen} needs a record key and none was given`)
  return scope
}

const SCREEN_LOADERS: Record<string, ScreenLoader> = {
  dashboard: async () => dashboardRows(await getDashboardSnapshot()),
  clients: async () =>
    (await getClientsPage({ sort: 'name', page: 1, pageSize: 50 })).rows.map(clientRows),
  deals: async (actor) => (await getDeals(actor)).map(dealRows),
  marketing: async () => marketingRows(await getMarketingContent()),
  'marketing-syndication': async () => countRows(await getMarketingDashboard(), 'syndication'),
  'property-admin': async () => (await getPropertyAdmin()).map(propertyRows),
  showings: async () => (await getShowings()).map(showingRows),
  storyboard: async () => ((await listStoryboardStories()) ?? []).map(storyboardRows),
  attention: async () => attentionRows(await getAttentionSnapshot()),
  'needs-review': async () => (await getNeedsReviewItems()).map(needsReviewRows),
  activity: async () => activityRows(await getActivityFeed(50)),
  'accounting-expenses': async () => (await getExpenses()).map(expenseRows),
  'accounting-receivables': async () => (await getReceivables()).map(receivableRows),
  'command-console': async () =>
    (await getIssueQueue({ pageSize: 50 })).rows.map((row) => ({
      id: row.id,
      cells: [row.title, row.type, row.severity, row.domainType],
      badge: row.state,
    })),
  tech: async () => healthRows(await getSystemHealth()),
  'tech-app-errors': async () => errorRows(25),
  'tech-flight-recorder': async () => traceRows({ limit: 50 }),
  'tech-runs': async () => (await listForgeBatches(10)).map(batchRows),

  // Record screens: opened from a row, and each one is about exactly one record.
  'client-record': async (_actor, scope) => clientRecordRows(requireScope(scope, 'client-record')),
  'deal-record': async (actor, scope) => dealRecordRows(requireScope(scope, 'deal-record'), actor),
  'property-record': async (_actor, scope) => propertyRecordRows(requireScope(scope, 'property-record')),
  'story-record': async (_actor, scope) => storyRecordRows(requireScope(scope, 'story-record')),
  'form-record': async (_actor, scope) =>
    factRowsFrom(await getFormInstance(requireScope(scope, 'form-record'))),

  // ---- the rest of the surfaces. Each reads the same thing its live page reads. Where a DTO's field names are not
  // known yet, `factRowsFrom` renders the read model's own field names rather than an invented column.
  'portal-root': async () => factRowsFrom(await getOpsCounts()),
  'accounting': async () => factRowsFrom(await getAccountingDashboard()),
  // The whole history, because a P&L with an unstated range is a table nobody can interpret.
  'accounting-pnl': async () =>
    factRowsFrom(await getPnlStatement('2020-01-01', new Date().toISOString().slice(0, 10))),
  'identity-quality': async () => factRowsFrom(await getIdentityQuality()),
  reporting: async () => factRowsFrom(await getReportingSnapshot()),
  'client-admin': async () => factRowsFrom(await getClientAdmin()),
  'media-admin': async () => factRowsFrom(await getMediaAdmin()),
  'property-media': async () => factRowsFrom(await getPropertyMediaCoverage()),
  'system-health': async () => healthRows(await getSystemHealth()),
  'db-test': async () => factRowsFrom(await getClients()),
  cabinet: async () => factRowsFrom(await listIssuedDocuments()),
  forms: async () => factRowsFrom(await listFormInstances()),
  // The screen key is `workflow-record`: an earlier draft used `workflow`, which wired the trace and left the screen
  // unreachable. Also the Flight Recorder chain, which the registry describes — a trace is opened from the story that
  // ran it, for the instance that actually ran it.
  'workflow-record': async (_actor, scope) =>
    traceRows({ workflowInstanceId: requireScope(scope, 'workflow-record'), limit: 200 }),
  'trace-record': async (_actor, scope) =>
    traceRows({ workflowInstanceId: requireScope(scope, 'trace-record'), limit: 200 }),
  'command-center': async () => factRowsFrom(await getFactoryCommandCenterSnapshot()),
  issues: async () => issueQueueRows(),
  'tech-kanban': async () => factRowsFrom((await listSprintRollups()) ?? []),
  'tech-line': async () => factRowsFrom(await listAgentWorkItems()),
  'whatsapp-meta': async () => metaWhatsappRows(),
  'whatsapp-coexistence': async () => coexistenceRows(),
  'design-lab': async () => designLabRows(),
  projects: async () => projectRows(),
  // The live page does exactly this: ask the workflow engine read service, but only when the engine is configured —
  // otherwise the live page renders an empty list too, so an unconfigured engine is an empty screen and not a failure.
  workflows: async () => (engineConfigured() ? factRowsFrom(await getWorkflowSummaries()) : []),
  security: async () => factRowsFrom(await getSecurityStatus()),
  'settings-users': async () => factRowsFrom(await getSettingsUsers()),
  'settings-roles': async () => factRowsFrom(await getSettingsRoles()),
  'settings-authorities': async () => factRowsFrom(await getSettingsAuthorities()),
}

async function GETHandler(req: NextRequest): Promise<Response> {
  let actor: ActingActor
  try {
    actor = await getPortalActingUser()
  } catch (error) {
    // Fail closed: a route that answers rows to anonymous callers is a data leak with extra steps.
    if (error instanceof AuthError) {
      return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })
    }
    throw error
  }

  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  // The record key, when the screen is about one record. The Rust side sends it with the effect rather than baking it
  // into the screen name, so one loader serves "a client" and the row decides which.
  const scope = req.nextUrl.searchParams.get('scope')
  const loader = SCREEN_LOADERS[screen]

  // An unknown or not-yet-wired screen is not an error: the Rust side asks for every screen it navigates to, and a
  // screen with no rows yet must render "Nothing to show yet" rather than a failure banner.
  if (!loader) {
    return NextResponse.json([])
  }

  return NextResponse.json(await loader(actor, scope))
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/rows', route: '/api/portal/rust-ui/rows' },
  GETHandler,
)

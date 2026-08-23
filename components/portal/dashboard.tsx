import Link from "next/link"

import { Panel } from "@/components/portal/panel"
import { TaskActions } from "@/components/portal/write/task-actions"
import type {
  Client,
  Deal,
  DealStage,
  InteractionChannel,
} from "@/lib/portal/types"
import type { DashboardSnapshot } from "@/db/dashboard"
import type { WorkflowSummary } from "@/workflow_app/read-service"

const stageOrder: DealStage[] = [
  "new_lead",
  "qualified",
  "showing",
  "offer",
  "under_contract",
  "closed",
]

function formatCurrency(value?: number) {
  if (!value) return "—"

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value)
}

function stageLabel(stage: DealStage) {
  switch (stage) {
    case "new_lead":
      return "New Lead"
    case "qualified":
      return "Qualified"
    case "showing":
      return "Showing"
    case "offer":
      return "Offer"
    case "under_contract":
      return "Under Contract"
    case "closed":
      return "Closed"
  }
}

function channelLabel(channel: InteractionChannel) {
  switch (channel) {
    case "website":
      return "Website"
    case "calendar":
      return "Calendar"
    case "document":
      return "Document"
    case "manual":
      return "Manual Entry"
    case "imessage":
      return "iMessage"
    case "sms":
      return "SMS"
    case "email":
      return "Email"
    case "call":
      return "Phone Call"
    case "meeting":
      return "Meeting"
    case "showing":
      return "Showing"
    case "whatsapp":
      return "WhatsApp"
    default:
      return "Note"
  }
}

function initials(name: string) {
  return name
    .split(" ")
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
}

const viewAllClass =
  "text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"

export function Dashboard({
  clients,
  deals,
  snapshot,
  workflowSummaries = [],
}: {
  clients: Client[]
  deals: Deal[]
  snapshot: DashboardSnapshot
  workflowSummaries?: WorkflowSummary[]
}) {
  const activeClients = clients.filter(
    (client) => client.status === "active" || client.status === "warm"
  )

  const activeDeals = deals.filter((deal) => deal.stage !== "closed")

  const attentionTasks = [
    ...snapshot.overdueTasks,
    ...snapshot.tasksDueSoon.filter(
      (task) => !snapshot.overdueTasks.some((overdue) => overdue.id === task.id)
    ),
  ].slice(0, 5)

  const overdueIds = new Set(snapshot.overdueTasks.map((task) => task.id))
  const todayTasks = [
    ...snapshot.overdueTasks.map((task) => ({ ...task, overdue: true as const })),
    ...snapshot.tasksDueSoon
      .filter((task) => !overdueIds.has(task.id))
      .map((task) => ({ ...task, overdue: false as const })),
  ].slice(0, 5)
  const recentInteractions = snapshot.recentInteractions.slice(0, 5)

  const featuredDeal =
    [...deals]
      .filter((deal) => deal.closingDate && deal.stage !== "closed")
      .sort((a, b) => String(a.closingDate).localeCompare(String(b.closingDate)))[0] ??
    deals.find((deal) => deal.stage === "showing") ??
    deals.find((deal) => deal.stage !== "closed") ??
    deals[0]

  const underContractCount = deals.filter(
    (deal) => deal.stage === "under_contract"
  ).length

  const activeWorkflows = workflowSummaries.filter((s) => s.outcome === null)
  const blockedWorkflows = activeWorkflows.filter((s) => s.blockerCount > 0)

  const kpis = [
    {
      label: "Clients",
      value: String(activeClients.length),
      href: "/portal/clients",
    },
    {
      label: "Live deals",
      value: String(activeDeals.length),
      href: "/portal/deals",
    },
    {
      label: "Upcoming",
      value: String(snapshot.tasksDueSoon.length),
      href: "/portal/attention",
    },
    {
      label: "Under contract",
      value: String(underContractCount),
      href: "/portal/deals",
    },
    {
      label: "In motion",
      value: String(activeWorkflows.length),
      href: "/portal/workflows",
    },
    {
      label: "Blocked",
      value: String(blockedWorkflows.length),
      href: "/portal/workflows",
      alert: blockedWorkflows.length > 0,
    },
  ]

  const liveDeals = deals.filter((deal) => deal.stage !== "closed")

  return (
    <div className="flex flex-col gap-4">
      <section className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
        <div className="grid grid-cols-2 sm:grid-cols-3 xl:grid-cols-6">
          {kpis.map((kpi) => (
            <Link
              key={kpi.label}
              href={kpi.href}
              className="px-4 py-3 transition hover:bg-white/25"
            >
              <div className="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                {kpi.label}
              </div>
              <div
                className={`mt-1 font-serif text-2xl font-light leading-none ${
                  kpi.alert
                    ? "text-[var(--portal-archive)]"
                    : "text-[var(--portal-navy)]"
                }`}
              >
                {kpi.value}
              </div>
            </Link>
          ))}
        </div>
      </section>

      <div className="grid gap-4 lg:grid-cols-3">
        <Panel
          variant="attention"
          compact
          heading="Needs attention"
          action={
            <Link href="/portal/attention" className={viewAllClass}>
              View all →
            </Link>
          }
          divider
          flush
        >
          {attentionTasks.length > 0 ? (
            attentionTasks.map((task) => (
              <div
                key={task.id}
                className="flex items-center gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0"
              >
                <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] text-[10px] font-medium text-[var(--portal-navy-soft)]">
                  {initials(task.contextName ?? "Task")}
                </div>
                <div className="min-w-0 flex-1">
                  {task.personId ? (
                    <Link
                      href={`/portal/clients/${task.personId}`}
                      className="block truncate text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                      {task.title}
                    </Link>
                  ) : (
                    <div className="truncate text-sm font-medium">{task.title}</div>
                  )}
                  <div className="truncate text-xs font-light text-black/45">
                    {task.contextName ?? "Task"}
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  <div className="text-right text-[11px] font-light text-black/40">
                    {task.dueAtLabel ?? "Unscheduled"}
                  </div>
                  <TaskActions taskId={task.id} compact />
                </div>
              </div>
            ))
          ) : (
            <EmptyState text="Nothing needs attention." />
          )}
        </Panel>

        <Panel
          variant="standard"
          compact
          heading="Today"
          action={
            <Link href="/portal/attention" className={viewAllClass}>
              View all →
            </Link>
          }
          divider
          flush
        >
          {todayTasks.length > 0 ? (
            todayTasks.map((task) => (
              <div
                key={task.id}
                className="flex items-start gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0"
              >
                <div
                  className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${
                    task.overdue
                      ? "bg-[var(--portal-archive)]"
                      : "bg-[var(--portal-navy)]"
                  }`}
                />
                <div className="min-w-0 flex-1">
                  {task.personId ? (
                    <Link
                      href={`/portal/clients/${task.personId}`}
                      className="block truncate text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                      {task.title}
                    </Link>
                  ) : (
                    <div className="truncate text-sm font-medium">{task.title}</div>
                  )}
                  <div className="truncate text-xs font-light text-black/45">
                    {task.overdue ? "Overdue · " : ""}
                    {task.dueAtLabel ?? "Unscheduled"}
                    {task.contextName ? ` · ${task.contextName}` : ""}
                  </div>
                </div>
              </div>
            ))
          ) : (
            <EmptyState text="Clear — nothing on the board today." />
          )}
        </Panel>

        <div className="flex flex-col gap-4">
          {featuredDeal ? (
            <section className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
              <div className="flex gap-3 p-4">
                {featuredDeal.heroMediaId ? (
                  <img
                    src={`/api/media/${featuredDeal.heroMediaId}`}
                    alt={featuredDeal.propertyName}
                    className="h-16 w-20 shrink-0 rounded-md object-cover"
                  />
                ) : (
                  <div className="h-16 w-20 shrink-0 rounded-md bg-gradient-to-br from-[var(--portal-blue-pale)] to-[var(--portal-navy-soft)]" />
                )}
                <div className="min-w-0 flex-1">
                  <p className="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                    Next closing
                  </p>
                  <Link
                    href={`/portal/deals/${featuredDeal.id}`}
                    className="mt-0.5 block truncate font-serif text-lg font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                  >
                    {featuredDeal.propertyName}
                  </Link>
                  <p className="truncate text-xs font-light text-black/45">
                    {formatCurrency(
                      featuredDeal.offerPrice ?? featuredDeal.listPrice
                    )}
                    {" · "}
                    {stageLabel(featuredDeal.stage)}
                  </p>
                </div>
              </div>
            </section>
          ) : (
            <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
              <p className="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                Next closing
              </p>
              <p className="mt-2 text-sm font-light text-black/40">
                No active deals.
              </p>
            </section>
          )}

          <Panel
            variant="standard"
            compact
            heading="Pipeline"
            action={
              <Link href="/portal/deals" className={viewAllClass}>
                View all →
              </Link>
            }
          >
            <div className="space-y-2">
              {stageOrder
                .filter((stage) => stage !== "closed")
                .map((stage) => {
                  const count = liveDeals.filter(
                    (deal) => deal.stage === stage
                  ).length
                  const percent =
                    liveDeals.length > 0
                      ? Math.round((count / liveDeals.length) * 100)
                      : 0

                  return (
                    <div key={stage} className="flex items-center gap-3">
                      <div className="w-[6.5rem] shrink-0 text-[10px] font-light uppercase tracking-[0.1em] text-black/50">
                        {stageLabel(stage)}
                      </div>
                      <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-[var(--portal-blue-pale)]">
                        <div
                          className="h-full bg-[var(--portal-navy)]"
                          style={{ width: `${percent}%` }}
                        />
                      </div>
                      <div className="w-4 shrink-0 text-right text-xs font-light tabular-nums text-black/45">
                        {count}
                      </div>
                    </div>
                  )
                })}
            </div>
          </Panel>
        </div>
      </div>

      <Panel
        variant="soft"
        compact
        heading="Recent activity"
        action={
          <Link href="/portal/activity" className={viewAllClass}>
            View all →
          </Link>
        }
        divider
        flush
      >
        {recentInteractions.length > 0 ? (
          recentInteractions.map((interaction) => (
            <div
              key={interaction.id}
              className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5 border-b border-[var(--portal-border)] px-4 py-2 last:border-b-0 sm:grid-cols-[7.5rem_5.5rem_1fr_auto]"
            >
              <div className="truncate text-xs font-light text-black/40">
                {interaction.occurredAtLabel}
              </div>
              <div className="hidden text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)] sm:block">
                {channelLabel(interaction.channel)}
              </div>
              <div className="min-w-0 truncate text-sm font-medium">
                {interaction.personName}
                <span className="font-light text-black/50">
                  {" — "}
                  {interaction.summary ?? interaction.title}
                </span>
              </div>
            </div>
          ))
        ) : (
          <EmptyState text="No recent relationship activity." />
        )}
      </Panel>
    </div>
  )
}

function EmptyState({
  text,
}: {
  text: string
}) {
  return (
    <div className="px-4 py-6 text-sm font-light text-black/40">{text}</div>
  )
}

import Link from "next/link"

import type {
  Client,
  Deal,
  DealStage,
  InteractionChannel,
} from "@/lib/portal/types"
import type { DashboardSnapshot } from "@/db/dashboard"

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
    default:
      return "Note"
  }
}

export function Dashboard({
  clients,
  deals,
  snapshot,
}: {
  clients: Client[]
  deals: Deal[]
  snapshot: DashboardSnapshot
}) {
  const activeClients = clients.filter(
    (client) => client.status === "active" || client.status === "warm"
  )

  const activeDeals = deals.filter(
    (deal) => deal.stage !== "closed"
  )

  const attentionTasks = [
    ...snapshot.overdueTasks,
    ...snapshot.tasksDueSoon,
  ].slice(0, 4)

  const calendarTasks = snapshot.tasksDueSoon.slice(0, 4)

  const recentInteractions = snapshot.recentInteractions

  const featuredDeal =
    deals.find((deal) => deal.stage === "showing") ??
    deals.find((deal) => deal.stage !== "closed") ??
    deals[0]

  const underContractCount = deals.filter(
    (deal) => deal.stage === "under_contract"
  ).length

  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Dashboard
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          What needs your attention, what is moving, and what happened recently — at a glance.
        </p>
      </div>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Active Clients"
          value={String(activeClients.length)}
          detail="Warm and active relationships"
        />

        <MetricCard
          label="Live Deals"
          value={String(activeDeals.length)}
          detail="Open opportunities"
        />

        <MetricCard
          label="Upcoming Actions"
          value={String(snapshot.tasksDueSoon.length)}
          detail="Tasks due within the next 7 days"
        />

        <MetricCard
          label="Under Contract"
          value={String(underContractCount)}
          detail="Transactions in progress"
        />
      </section>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_360px]">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="flex items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
                Needs Attention
              </p>

              <h2 className="mt-2 font-serif text-2xl font-light">
                Client Follow-Up
              </h2>
            </div>

            <Link
              href="/portal/attention"
              className="mt-1 inline-flex shrink-0 items-center gap-1.5 text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
            >
              View all
              <span aria-hidden>→</span>
            </Link>
          </div>

          <div>
            {attentionTasks.length > 0 ? (
              attentionTasks.map((task) => (
                <div
                  key={task.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="flex items-start gap-4">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] font-serif text-sm font-light text-[var(--portal-navy-soft)]">
                      {(task.contextName ?? "Task")
                        .split(" ")
                        .slice(0, 2)
                        .map((word) => word[0])
                        .join("")}
                    </div>

                    <div className="min-w-0 flex-1">
                      <div className="flex justify-between gap-4">
                        <div className="text-sm font-medium">
                          {task.contextName ?? "Task"}
                        </div>

                        <div className="shrink-0 text-xs font-light text-black/40">
                          {task.dueAtLabel ?? "Unscheduled"}
                        </div>
                      </div>

                      {task.personId ? (
                        <Link
                          href={`/portal/clients/${task.personId}`}
                          className="mt-1 inline-flex min-h-11 items-center font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                        >
                          {task.title}
                        </Link>
                      ) : (
                        <div className="mt-1 font-serif text-lg font-light">
                          {task.title}
                        </div>
                      )}

                      {task.detail && (
                        <div className="mt-1 text-xs font-light text-black/45">
                          {task.detail}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))
            ) : (
              <EmptyState text="No follow-ups need attention." />
            )}
          </div>
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="flex items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
                Upcoming
              </p>

              <h2 className="mt-2 font-serif text-2xl font-light">
                Next on the Calendar
              </h2>
            </div>

            <Link
              href="/portal/attention"
              className="mt-1 inline-flex shrink-0 items-center gap-1.5 text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
            >
              View all
              <span aria-hidden>→</span>
            </Link>
          </div>

          <div className="px-6">
            {calendarTasks.length > 0 ? (
              calendarTasks.map((task, index) => (
                <div
                  key={task.id}
                  className="grid grid-cols-[24px_1fr] gap-4 border-b border-[var(--portal-border)] py-5 last:border-b-0"
                >
                  <div className="relative flex justify-center">
                    <div className="mt-1.5 h-2 w-2 rounded-full bg-[var(--portal-navy)]" />

                    {index < calendarTasks.length - 1 && (
                      <div className="absolute bottom-[-20px] top-4 w-px bg-[var(--portal-border)]" />
                    )}
                  </div>

                  <div>
                    <div className="text-xs font-light text-black/40">
                      {task.dueAtLabel ?? "Unscheduled"}
                    </div>

                    {task.personId ? (
                      <Link
                        href={`/portal/clients/${task.personId}`}
                        className="mt-1 inline-flex min-h-11 items-center text-sm font-medium text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                      >
                        {task.title}
                      </Link>
                    ) : (
                      <div className="mt-1 text-sm font-medium">
                        {task.title}
                      </div>
                    )}

                    <div className="mt-1 text-xs font-light text-black/45">
                      {task.contextName ?? "Task"}
                      {task.detail && ` · ${task.detail}`}
                    </div>
                  </div>
                </div>
              ))
            ) : (
              <div className="py-10 text-sm font-light text-black/40">
                Nothing due in the next 7 days.
              </div>
            )}
          </div>
        </section>

        {featuredDeal ? (
          <section className="overflow-hidden rounded-sm border border-[var(--portal-border)] bg-white">
            {featuredDeal.heroMediaId ? (
              <img
                src={`/api/media/${featuredDeal.heroMediaId}`}
                alt={featuredDeal.propertyName}
                className="h-52 w-full object-cover"
              />
            ) : (
              <div className="h-52 bg-gradient-to-br from-[var(--portal-blue-pale)] via-[#b9c9d5] to-[var(--portal-navy-soft)]" />
            )}

            <div className="p-6">
              <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
                Featured Opportunity
              </p>

              <Link
                href={`/portal/deals/${featuredDeal.id}`}
                className="mt-3 inline-flex min-h-11 items-center font-serif text-2xl font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
              >
                {featuredDeal.propertyName}
              </Link>

              <p className="mt-1 text-xs font-light text-black/45">
                {featuredDeal.propertyLocation}
                {featuredDeal.propertyDescriptor &&
                  ` · ${featuredDeal.propertyDescriptor}`}
              </p>

              <div className="mt-6 border-t border-[var(--portal-border)] pt-5">
                <div className="font-serif text-2xl font-light">
                  {formatCurrency(
                    featuredDeal.offerPrice ??
                      featuredDeal.listPrice
                  )}
                </div>

                <div className="mt-2 text-xs font-light text-black/45">
                  {featuredDeal.clientName}
                </div>

                <div className="mt-4 inline-flex rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                  {stageLabel(featuredDeal.stage)}
                </div>
              </div>
            </div>
          </section>
        ) : (
          <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
            <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
              Featured Opportunity
            </p>

            <div className="mt-8 font-serif text-2xl font-light">
              No active deals
            </div>

            <p className="mt-2 text-sm font-light text-black/40">
              Featured opportunities will appear here.
            </p>
          </section>
        )}
      </div>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
                Portfolio Snapshot
              </p>

              <h2 className="mt-2 font-serif text-2xl font-light">
                Deal Pipeline
              </h2>
            </div>

            <Link
              href="/portal/deals"
              className="mt-1 inline-flex shrink-0 items-center gap-1.5 text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
            >
              View all
              <span aria-hidden>→</span>
            </Link>
          </div>

          <div className="mt-7 space-y-4">
            {stageOrder.map((stage) => {
              const count = deals.filter(
                (deal) => deal.stage === stage
              ).length

              const percent =
                deals.length > 0
                  ? Math.round((count / deals.length) * 100)
                  : 0

              return (
                <div key={stage}>
                  <div className="mb-2 flex items-center justify-between">
                    <div className="text-xs font-light uppercase tracking-[0.12em] text-black/50">
                      {stageLabel(stage)}
                    </div>

                    <div className="text-xs font-light text-black/40">
                      {count}
                    </div>
                  </div>

                  <div className="h-1.5 overflow-hidden bg-[var(--portal-blue-pale)]">
                    <div
                      className="h-full bg-[var(--portal-navy)]"
                      style={{ width: `${percent}%` }}
                    />
                  </div>
                </div>
              )
            })}
          </div>
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="flex items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
                Recent Activity
              </p>

              <h2 className="mt-2 font-serif text-2xl font-light">
                Relationship Timeline
              </h2>
            </div>

            <Link
              href="/portal/activity"
              className="mt-1 inline-flex shrink-0 items-center gap-1.5 text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
            >
              View all
              <span aria-hidden>→</span>
            </Link>
          </div>

          <div>
            {recentInteractions.length > 0 ? (
              recentInteractions.map((interaction) => (
                <div
                  key={interaction.id}
                  className="grid gap-3 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0 md:grid-cols-[130px_110px_1fr]"
                >
                  <div className="text-xs font-light text-black/40">
                    {interaction.occurredAtLabel}
                  </div>

                  <div className="text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                    {channelLabel(interaction.channel)}
                  </div>

                  <div>
                    <div className="text-sm font-medium">
                      {interaction.personName}
                    </div>

                    <div className="mt-1 text-sm font-light text-black/55">
                      {interaction.summary ?? interaction.title}
                    </div>
                  </div>
                </div>
              ))
            ) : (
              <EmptyState text="No recent relationship activity." />
            )}
          </div>
        </section>
      </div>
    </div>
  )
}

function MetricCard({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail: string
}) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        {label}
      </div>

      <div className="mt-4 font-serif text-3xl font-light text-[var(--portal-navy)]">
        {value}
      </div>

      <div className="mt-2 text-xs font-light text-black/40">
        {detail}
      </div>
    </div>
  )
}

function EmptyState({
  text,
}: {
  text: string
}) {
  return (
    <div className="px-6 py-10 text-sm font-light text-black/40">
      {text}
    </div>
  )
}

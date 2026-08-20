import Link from "next/link"

import {
  CreateShowingPanel,
  ShowingActions,
} from "@/components/portal/write/showing-actions"
import type { ShowingPropertyOption } from "@/components/portal/write/showing-actions"
import type { Showing, ShowingStatus } from "@/db/showings"

function statusLabel(status: ShowingStatus) {
  switch (status) {
    case "requested":
      return "Requested"
    case "scheduled":
      return "Scheduled"
    case "completed":
      return "Completed"
    case "cancelled":
      return "Cancelled"
  }
}

function SectionCard({
  title,
  subtitle,
  count,
  children,
}: {
  title: string
  subtitle: string
  count: number
  children: React.ReactNode
}) {
  return (
    <section className="rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light">{title}</h2>
          <p className="mt-1 text-xs font-light text-black/40">{subtitle}</p>
        </div>
        <span className="text-xs font-light text-black/35">{count}</span>
      </div>
      {children}
    </section>
  )
}

function Empty({ text }: { text: string }) {
  return <div className="px-6 py-10 text-sm font-light text-black/40">{text}</div>
}

function ShowingRow({ showing }: { showing: Showing }) {
  const propertyName = showing.propertyName ?? showing.dealPropertyName

  return (
    <div className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <Link
            href={`/portal/clients/${showing.personId}`}
            className="inline-flex min-h-11 items-center font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
          >
            {showing.personName}
          </Link>
          <div className="mt-1 text-xs font-light text-black/45">
            {propertyName ?? "—"}
            {showing.dealId && (
              <Link
                href={`/portal/deals/${showing.dealId}`}
                className="ml-2 text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
              >
                View deal
              </Link>
            )}
          </div>
          {showing.feedback && (
            <p className="mt-2 text-sm font-light leading-6 text-black/55">
              {showing.feedback}
            </p>
          )}
        </div>
        <div className="shrink-0 text-right text-xs font-light text-black/40">
          <div>Requested {showing.requestedAtLabel}</div>
          {showing.scheduledAtLabel && (
            <div>Scheduled {showing.scheduledAtLabel}</div>
          )}
          {showing.completedAtLabel && (
            <div>Completed {showing.completedAtLabel}</div>
          )}
          {showing.cancelledAtLabel && (
            <div>Cancelled {showing.cancelledAtLabel}</div>
          )}
        </div>
      </div>
      <div className="mt-3">
        <ShowingActions showingId={showing.id} status={showing.status} />
      </div>
    </div>
  )
}

export function Showings({
  showings,
  properties,
}: {
  showings: Showing[]
  properties: ShowingPropertyOption[]
}) {
  const requested = showings.filter((showing) => showing.status === "requested")
  const scheduled = showings.filter((showing) => showing.status === "scheduled")
  const completed = showings.filter((showing) => showing.status === "completed")
  const cancelled = showings.filter((showing) => showing.status === "cancelled")

  const groups: Array<{
    key: ShowingStatus
    title: string
    subtitle: string
    items: Showing[]
  }> = [
    {
      key: "scheduled",
      title: "Upcoming Scheduled",
      subtitle: "Showings scheduled on the calendar",
      items: scheduled,
    },
    {
      key: "requested",
      title: "Requested / Unscheduled",
      subtitle: "Showings requested but not yet scheduled",
      items: requested,
    },
    {
      key: "completed",
      title: "Recently Completed",
      subtitle: "Completed showings and any feedback",
      items: completed,
    },
    {
      key: "cancelled",
      title: "Cancelled",
      subtitle: "Showings that did not proceed",
      items: cancelled,
    },
  ]

  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Operations & Reporting
        </p>
        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Showings
        </h1>
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Showing lifecycle records — requested, scheduled, completed, and
          cancelled — drawn from the canonical showing table.
        </p>
      </div>

      <section className="mb-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="border-b border-[var(--portal-border)] px-6 py-5">
          <h2 className="font-serif text-2xl font-light">Request a showing</h2>
          <p className="mt-1 text-xs font-light text-black/40">
            Create a requested showing for an existing person and property.
          </p>
        </div>
        <div className="px-6 py-5">
          <CreateShowingPanel properties={properties} />
        </div>
      </section>

      {showings.length === 0 ? (
        <div className="rounded-sm border border-[var(--portal-border)] bg-white p-10">
          <p className="text-sm font-light text-black/45">
            No showings on record yet. Showings will appear here as they are
            requested and scheduled.
          </p>
        </div>
      ) : (
        <div className="grid gap-6 2xl:grid-cols-2">
          {groups.map((group) => (
            <SectionCard
              key={group.key}
              title={group.title}
              subtitle={group.subtitle}
              count={group.items.length}
            >
              {group.items.length > 0 ? (
                <div>
                  {group.items.map((showing) => (
                    <ShowingRow key={showing.id} showing={showing} />
                  ))}
                </div>
              ) : (
                <Empty text={`No ${statusLabel(group.key).toLowerCase()} showings.`} />
              )}
            </SectionCard>
          ))}
        </div>
      )}
    </div>
  )
}

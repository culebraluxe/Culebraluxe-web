import Link from "next/link"

import { Panel } from "@/components/portal/panel"
import { TaskActions } from "@/components/portal/write/task-actions"
import type { AttentionSnapshot } from "@/db/attention"
import type { ActivityFeedEntry } from "@/db/activity-feed"
import type { Showing, ShowingStatus } from "@/db/showings"

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function showingStatusLabel(status: ShowingStatus) {
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

function channelLabel(channel: string) {
  switch (channel) {
    case "website":
      return "Website"
    case "email":
      return "Email"
    case "call":
      return "Phone Call"
    case "imessage":
      return "iMessage"
    case "sms":
      return "SMS"
    case "meeting":
      return "Meeting"
    case "showing":
      return "Showing"
    case "document":
      return "Document"
    case "manual":
      return "Manual"
    case "whatsapp":
      return "WhatsApp"
    default:
      return channel.charAt(0).toUpperCase() + channel.slice(1)
  }
}

function count(actionCount: number) {
  return (
    <span className="text-xs font-light text-black/40">
      {actionCount} {actionCount === 1 ? "item" : "items"}
    </span>
  )
}

function emptyState(text: string) {
  return <div className="px-4 py-6 text-sm font-light text-black/40">{text}</div>
}

type QueueTask = AttentionSnapshot["overdueTasks"][number] & {
  bucket: "overdue" | "soon"
}

export function Attention({
  snapshot,
  showings,
  activity,
}: {
  snapshot: AttentionSnapshot
  showings: Showing[]
  activity: ActivityFeedEntry[]
}) {
  const overdueIds = new Set(snapshot.overdueTasks.map((task) => task.id))
  const queue: QueueTask[] = [
    ...snapshot.overdueTasks.map((task) => ({ ...task, bucket: "overdue" as const })),
    ...snapshot.dueSoonTasks
      .filter((task) => !overdueIds.has(task.id))
      .map((task) => ({ ...task, bucket: "soon" as const })),
  ]

  // Active showings (requested or scheduled) form the showings queue.
  const activeShowings = showings.filter(
    (showing) =>
      showing.status === "scheduled" || showing.status === "requested",
  )

  const recentActivity = activity.slice(0, 10)

  return (
    <div className="flex flex-col gap-4">
      {/* Top row: follow-up queue + showings queue. */}
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel
          compact
          variant="attention"
          heading="Follow-up queue"
          action={count(queue.length)}
          divider
          flush
        >
          {queue.length > 0 ? (
            queue.map((task) => (
              <div
                key={task.id}
                className="flex flex-wrap items-center gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0"
              >
                <span
                  className={[
                    "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-light uppercase tracking-[0.1em]",
                    task.bucket === "overdue"
                      ? "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]"
                      : "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
                  ].join(" ")}
                >
                  {task.bucket === "overdue" ? "Overdue" : "Soon"}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-[var(--portal-navy)]">
                    {task.title}
                  </div>
                  <div className="truncate text-xs font-light text-black/45">
                    {task.dueAtLabel ?? "Unscheduled"}
                    {task.personName ? ` · ${task.personName}` : ""}
                    {(task.propertyName || task.dealPropertyName)
                      ? ` · ${task.dealPropertyName ?? task.propertyName}`
                      : ""}
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {task.personId ? (
                    <Link
                      href={`/portal/clients/${task.personId}`}
                      className="inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] hover:text-[var(--portal-navy)]"
                    >
                      Open
                    </Link>
                  ) : null}
                  <TaskActions taskId={task.id} compact />
                </div>
              </div>
            ))
          ) : (
            emptyState("Queue is clear.")
          )}
        </Panel>

        <Panel
          compact
          heading="Showings"
          action={count(activeShowings.length)}
          divider
          flush
        >
          {activeShowings.length > 0 ? (
            activeShowings.map((showing) => (
              <div
                key={showing.id}
                className="flex flex-wrap items-center gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0"
              >
                <div className="min-w-0 flex-1">
                  <Link
                    href={`/portal/clients/${showing.personId}`}
                    className="truncate text-sm font-medium text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                  >
                    {showing.personName}
                  </Link>
                  <div className="truncate text-xs font-light text-black/45">
                    {showing.dealPropertyName ?? showing.propertyName ?? "—"}
                  </div>
                </div>
                <div className="shrink-0 text-right text-xs font-light text-black/40">
                  <div>{showingStatusLabel(showing.status)}</div>
                  <div>{showing.scheduledAtLabel ?? showing.requestedAtLabel}</div>
                </div>
              </div>
            ))
          ) : (
            emptyState("No active showings.")
          )}
        </Panel>
      </div>

      {/* Second row: open queue + quiet-but-important. */}
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel
          compact
          heading="Open queue"
          action={count(snapshot.peopleWithOpenWork.length)}
          divider
          flush
        >
          {snapshot.peopleWithOpenWork.length > 0 ? (
            snapshot.peopleWithOpenWork.map((person) => (
              <Link
                key={person.id}
                href={`/portal/clients/${person.id}`}
                className="flex items-center justify-between gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0 hover:bg-white/25"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-[var(--portal-navy)]">
                    {person.displayName}
                  </div>
                  <div className="text-xs font-light text-black/45">
                    {roleLabel(person.role)} · {statusLabel(person.status)}
                  </div>
                </div>
                <div className="shrink-0 text-right text-xs font-light text-black/40">
                  {person.openTaskCount} open
                </div>
              </Link>
            ))
          ) : (
            emptyState("No relationships have open tasks.")
          )}
        </Panel>


        <Panel
          compact
          heading="Quiet but important"
          action={count(snapshot.quietButImportant.length)}
          divider
          flush
        >
          {snapshot.quietButImportant.length > 0 ? (
            snapshot.quietButImportant.map((person) => (
              <Link
                key={person.id}
                href={`/portal/clients/${person.id}`}
                className="flex items-center justify-between gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0 hover:bg-white/25"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-[var(--portal-navy)]">
                    {person.displayName}
                  </div>
                  <div className="text-xs font-light text-black/45">
                    {person.lastContactLabel
                      ? `Last: ${person.lastContactLabel}`
                      : "No contact recorded"}
                  </div>
                </div>
                <div className="shrink-0 text-right text-xs font-light text-black/40">
                  {person.activeDealCount} deal
                  {person.activeDealCount === 1 ? "" : "s"}
                </div>
              </Link>
            ))
          ) : (
            emptyState("Nobody has gone quiet.")
          )}
        </Panel>
      </div>

      {/* Activity across the bottom width. */}
      <Panel heading="Activity" action={count(activity.length)} divider flush>
        {recentActivity.length > 0 ? (
          recentActivity.map((entry) => (
            <div
              key={entry.id}
              className="flex flex-wrap items-center gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0"
            >
              <span className="w-20 shrink-0 text-xs font-light text-black/40">
                {entry.occurredAtLabel}
              </span>
              <span className="w-24 shrink-0 text-xs font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                {channelLabel(entry.channel)}
              </span>
              <div className="min-w-0 flex-1">
                {entry.personId ? (
                  <Link
                    href={`/portal/clients/${entry.personId}`}
                    className="truncate text-sm font-medium text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                  >
                    {entry.personName ?? "Unknown person"}
                  </Link>
                ) : (
                  <div className="truncate text-sm font-medium">
                    {entry.personName ?? "Unknown person"}
                  </div>
                )}
                <div className="truncate text-xs font-light text-black/45">
                  {entry.summary ?? entry.title ?? "Interaction"}
                </div>
              </div>
            </div>
          ))
        ) : (
          emptyState("No activity recorded yet.")
        )}
      </Panel>
    </div>
  )
}

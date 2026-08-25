import Link from "next/link"

import { Panel } from "@/components/portal/panel"
import { ActivityTimeline } from "@/components/portal/ui/portal-timeline"
import { TaskActions } from "@/components/portal/write/task-actions"
import type { AttentionSnapshot, PersonRelationshipContext } from "@/db/attention"
import type { ActivityFeedEntry } from "@/db/activity-feed"
import type { Showing, ShowingStatus } from "@/db/showings"
import { ContactActions } from "@/components/portal/contact-actions"
import { FollowUpActions } from "@/components/portal/follow-up-actions"

function sourceLabel(source: string): string {
  return source === "apple_contacts" ? "Apple" : source === "gmail_contacts" ? "Gmail" : source
}

// REL-INTEL — deterministic, human-readable relationship context line.
function relationshipLine(ctx: PersonRelationshipContext | undefined): string | null {
  if (!ctx || !ctx.hasEvidence) return null
  if (ctx.reason) return ctx.reason
  const src = ctx.sources.map(sourceLabel).join(" · ")
  return src ? `Sources: ${src}` : null
}

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
  relationshipContext,
  contactEvidence,
}: {
  snapshot: AttentionSnapshot
  showings: Showing[]
  activity: ActivityFeedEntry[]
  relationshipContext?: Record<string, PersonRelationshipContext>
  contactEvidence?: Record<string, { emails: string[]; phones: string[] }>
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
                {task.personId ? (
                  <div className="flex w-full flex-wrap items-center gap-3 border-t border-[var(--portal-border)]/50 pt-2">
                    <ContactActions
                      evidence={contactEvidence?.[task.personId] ?? { emails: [], phones: [] }}
                      personName={task.personName}
                    />
                    <FollowUpActions
                      followUpId={task.id}
                      personId={task.personId}
                      personName={task.personName}
                    />
                  </div>
                ) : null}
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

      {(() => {
        const anyLimited = Object.values(relationshipContext ?? {}).some((c) => c.coverageLimited)
        return anyLimited ? (
          <p className="rounded-[var(--portal-panel-radius)] portal-glass-panel px-4 py-3 text-[11px] font-light text-black/40">
            Relationship context reflects a bounded, partial email census — it may understate
            recent contact and is never treated as a complete history.
          </p>
        ) : null
      })()}

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
                  {(() => {
                    const line = relationshipLine(relationshipContext?.[person.id])
                    return line ? (
                      <div className="mt-0.5 truncate text-[11px] font-light text-black/35">
                        {line}
                      </div>
                    ) : null
                  })()}
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
                  {(() => {
                    const line = relationshipLine(relationshipContext?.[person.id])
                    return line ? (
                      <div className="mt-0.5 truncate text-[11px] font-light text-black/35">
                        {line}
                      </div>
                    ) : null
                  })()}
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
          <div className="px-4 py-4 sm:px-6">
            <ActivityTimeline
              items={recentActivity.map((entry) => ({
                id: entry.id,
                actor: entry.personName ?? "Unknown person",
                timestamp: entry.occurredAtLabel,
                text: entry.summary ?? entry.title ?? "Interaction",
                detail: (
                  <span className="inline-flex flex-wrap items-center gap-x-3 gap-y-1">
                    <span className="uppercase tracking-[0.12em]">
                      {channelLabel(entry.channel)}
                    </span>
                    {entry.personId ? (
                      <Link
                        href={`/portal/clients/${entry.personId}`}
                        className="text-[var(--portal-navy)] underline underline-offset-2 hover:text-[var(--portal-navy-soft)]"
                      >
                        Open client
                      </Link>
                    ) : null}
                  </span>
                ),
              }))}
            />
          </div>
        ) : (
          emptyState("No activity recorded yet.")
        )}
      </Panel>
    </div>
  )
}

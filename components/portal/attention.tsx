import Link from "next/link"

import { PageHeader } from "@/components/portal/page-header"
import { Panel } from "@/components/portal/panel"
import { TaskActions } from "@/components/portal/write/task-actions"
import type { AttentionSnapshot } from "@/db/attention"

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

type QueueTask = AttentionSnapshot["overdueTasks"][number] & {
  bucket: "overdue" | "soon"
}

export function Attention({
  snapshot,
}: {
  snapshot: AttentionSnapshot
}) {
  const overdueIds = new Set(snapshot.overdueTasks.map((task) => task.id))
  const queue: QueueTask[] = [
    ...snapshot.overdueTasks.map((task) => ({ ...task, bucket: "overdue" as const })),
    ...snapshot.dueSoonTasks
      .filter((task) => !overdueIds.has(task.id))
      .map((task) => ({ ...task, bucket: "soon" as const })),
  ]

  return (
    <div className="flex flex-col gap-4">
      <PageHeader compact eyebrow="Queue" title="Attention">
        <span className="text-xs font-light text-black/40">
          {queue.length} {queue.length === 1 ? "item" : "items"}
        </span>
      </PageHeader>

      <Panel compact variant="attention" heading="Follow-up queue" divider flush>
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
          <div className="px-4 py-8 text-sm font-light text-black/40">
            Queue is clear.
          </div>
        )}
      </Panel>

      <div className="grid gap-4 lg:grid-cols-2">
        <Panel
          compact
          heading="People with open work"
          action={
            <span className="text-xs font-light text-black/35">
              {snapshot.peopleWithOpenWork.length}
            </span>
          }
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
            <div className="px-4 py-6 text-sm font-light text-black/40">
              No relationships have open tasks.
            </div>
          )}
        </Panel>

        <Panel
          compact
          heading="Quiet but important"
          action={
            <span className="text-xs font-light text-black/35">
              {snapshot.quietButImportant.length}
            </span>
          }
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
            <div className="px-4 py-6 text-sm font-light text-black/40">
              Nobody has gone quiet.
            </div>
          )}
        </Panel>
      </div>
    </div>
  )
}

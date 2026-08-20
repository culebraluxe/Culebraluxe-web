import Link from "next/link"

import type { AttentionSnapshot } from "@/db/attention"

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function channelLabel(channel: string | null) {
  switch (channel) {
    case "call":
      return "Phone Call"
    case "email":
      return "Email"
    case "imessage":
      return "iMessage"
    case "sms":
      return "SMS"
    case "meeting":
      return "Meeting"
    case "showing":
      return "Showing"
    case "website":
      return "Website"
    default:
      return channel ? channel.charAt(0).toUpperCase() + channel.slice(1) : null
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

export function Attention({
  snapshot,
}: {
  snapshot: AttentionSnapshot
}) {
  const totalAttention =
    snapshot.overdueTasks.length + snapshot.dueSoonTasks.length

  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Attention
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Follow-up work that matters right now — derived from open tasks,
          activity, and relationships already on file.
        </p>
      </div>

      <div className="mb-6 flex items-center justify-between border-b border-[var(--portal-border)] pb-5">
        <p className="text-xs font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
          Follow-Up Queue
        </p>

        <p className="text-xs font-light text-black/40">
          {totalAttention} {totalAttention === 1 ? "task" : "tasks"} due or overdue
        </p>
      </div>

      <div className="grid gap-6 2xl:grid-cols-2">
        <SectionCard
          title="Overdue"
          subtitle="Open tasks past their due date"
          count={snapshot.overdueTasks.length}
        >
          {snapshot.overdueTasks.length > 0 ? (
            <div>
              {snapshot.overdueTasks.map((task) => (
                <div
                  key={task.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="flex items-start justify-between gap-4">
                    {task.personId ? (
                      <Link
                        href={`/portal/clients/${task.personId}`}
                        className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                      >
                        {task.title}
                      </Link>
                    ) : (
                      <div className="font-serif text-lg font-light">
                        {task.title}
                      </div>
                    )}
                    <span className="rounded-full bg-[#f3e3d8] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[#8a4b2a]">
                      Overdue
                    </span>
                  </div>
                  {task.detail && (
                    <p className="mt-1 text-sm font-light text-black/50">{task.detail}</p>
                  )}
                  <div className="mt-2 text-xs font-light text-black/40">
                    {task.dueAtLabel ?? "Unscheduled"}
                    {task.personName && ` · ${task.personName}`}
                    {(task.propertyName || task.dealPropertyName) && ` · ${
                      task.dealPropertyName ?? task.propertyName
                    }`}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="Nothing is overdue." />
          )}
        </SectionCard>

        <SectionCard
          title="Due Soon"
          subtitle="Open tasks due within the next 7 days"
          count={snapshot.dueSoonTasks.length}
        >
          {snapshot.dueSoonTasks.length > 0 ? (
            <div>
              {snapshot.dueSoonTasks.map((task) => (
                <div
                  key={task.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  {task.personId ? (
                    <Link
                      href={`/portal/clients/${task.personId}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {task.title}
                    </Link>
                  ) : (
                    <div className="font-serif text-lg font-light">{task.title}</div>
                  )}
                  {task.detail && (
                    <p className="mt-1 text-sm font-light text-black/50">{task.detail}</p>
                  )}
                  <div className="mt-2 text-xs font-light text-black/40">
                    {task.dueAtLabel ?? "Unscheduled"}
                    {task.personName && ` · ${task.personName}`}
                    {(task.propertyName || task.dealPropertyName) && ` · ${
                      task.dealPropertyName ?? task.propertyName
                    }`}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="No tasks due within the next 7 days." />
          )}
        </SectionCard>
      </div>

      <div className="mt-6 grid gap-6 2xl:grid-cols-2">
        <SectionCard
          title="People with Open Work"
          subtitle="Relationships carrying at least one open task"
          count={snapshot.peopleWithOpenWork.length}
        >
          {snapshot.peopleWithOpenWork.length > 0 ? (
            <div>
              {snapshot.peopleWithOpenWork.map((person) => (
                <div
                  key={person.id}
                  className="flex items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="min-w-0">
                    <Link
                      href={`/portal/clients/${person.id}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {person.displayName}
                    </Link>
                    <div className="mt-1 text-xs font-light text-black/45">
                      {roleLabel(person.role)} · {statusLabel(person.status)}
                    </div>
                  </div>
                  <div className="text-right text-xs font-light text-black/40">
                    <div className="text-sm font-medium text-[var(--portal-navy)]">
                      {person.openTaskCount} open
                    </div>
                    <div className="mt-1">
                      {person.lastContactLabel
                        ? `Last: ${person.lastContactLabel}${
                            channelLabel(person.lastContactChannel)
                              ? ` · ${channelLabel(person.lastContactChannel)}`
                              : ""
                          }`
                        : "No contact recorded"}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="No relationships have open tasks." />
          )}
        </SectionCard>

        <SectionCard
          title="Quiet but Important"
          subtitle="Active relationship with no contact in the last 30 days"
          count={snapshot.quietButImportant.length}
        >
          {snapshot.quietButImportant.length > 0 ? (
            <div>
              {snapshot.quietButImportant.map((person) => (
                <div
                  key={person.id}
                  className="flex items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="min-w-0">
                    <Link
                      href={`/portal/clients/${person.id}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {person.displayName}
                    </Link>
                    <div className="mt-1 text-xs font-light text-black/45">
                      {roleLabel(person.role)} · {statusLabel(person.status)}
                    </div>
                  </div>
                  <div className="text-right text-xs font-light text-black/40">
                    <div className="text-sm font-medium text-[var(--portal-navy)]">
                      {person.activeDealCount} deal{person.activeDealCount === 1 ? "" : "s"} ·{" "}
                      {person.openTaskCount} task{person.openTaskCount === 1 ? "" : "s"}
                    </div>
                    <div className="mt-1">
                      {person.lastContactLabel
                        ? `Last contact: ${person.lastContactLabel}`
                        : "No contact recorded"}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="No quiet-but-important relationships right now." />
          )}
        </SectionCard>
      </div>
    </div>
  )
}

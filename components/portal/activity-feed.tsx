import Link from "next/link"

import type { ActivityFeedEntry } from "@/db/activity-feed"

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
      return "Manual Entry"
    case "whatsapp":
      return "WhatsApp"
    default:
      return channel.charAt(0).toUpperCase() + channel.slice(1)
  }
}

export function ActivityFeed({
  entries,
}: {
  entries: ActivityFeedEntry[]
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Activity
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Every interaction across the book, ordered by when it happened.
        </p>
      </div>

      <section className="overflow-hidden rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">
              Unified Activity Feed
            </h2>

            <p className="mt-1 text-xs font-light text-black/40">
              Calls, email, messages, meetings, showings, website and notes.
            </p>
          </div>

          <div className="text-xs font-light text-black/35">
            {entries.length} entries
          </div>
        </div>

        {entries.length > 0 ? (
          <div>
            {entries.map((entry) => (
              <div
                key={entry.id}
                className="grid gap-3 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0 md:grid-cols-[150px_130px_1fr]"
              >
                <div className="text-xs font-light text-black/40">
                  {entry.occurredAtLabel}
                </div>

                <div className="text-xs font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                  {channelLabel(entry.channel)}
                  {entry.direction && (
                    <span className="ml-2 normal-case text-black/35">
                      {entry.direction}
                    </span>
                  )}
                </div>

                <div>
                  {entry.personId ? (
                    <Link
                      href={`/portal/clients/${entry.personId}`}
                      className="inline-flex min-h-11 items-center text-sm font-medium text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {entry.personName ?? "Unknown person"}
                    </Link>
                  ) : (
                    <div className="text-sm font-medium">
                      {entry.personName ?? "Unknown person"}
                    </div>
                  )}

                  <div className="mt-1 text-sm font-light text-black/55">
                    {entry.summary ?? entry.title ?? "Interaction"}
                  </div>

                  {(entry.propertyName || entry.dealPropertyName) && (
                    <div className="mt-1 text-xs font-light text-black/40">
                      {entry.dealPropertyName && entry.dealId ? (
                        <Link
                          href={`/portal/deals/${entry.dealId}`}
                          className="text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                        >
                          {entry.dealPropertyName}
                        </Link>
                      ) : (
                        entry.dealPropertyName ?? entry.propertyName
                      )}
                      {entry.propertyName &&
                        entry.dealPropertyName &&
                        entry.propertyName !== entry.dealPropertyName &&
                        ` · ${entry.propertyName}`}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="px-6 py-12 text-sm font-light text-black/40">
            No activity recorded yet. Interactions will appear here as they are captured.
          </div>
        )}
      </section>
    </div>
  )
}

import type { NeedsReviewItem } from "@/db/needs-review"

function requestTypeLabel(
  requestType: NeedsReviewItem["requestType"]
) {
  return requestType === "private_viewing"
    ? "Private Viewing"
    : "Property Information"
}

function statusLabel(status: NeedsReviewItem["status"]) {
  switch (status) {
    case "received":
      return "Received"
    case "processing":
      return "Processing"
    case "resolution_required":
      return "Needs Review"
    default:
      return status
  }
}

function statusClasses(status: NeedsReviewItem["status"]) {
  switch (status) {
    case "resolution_required":
      return "bg-[#f3e3d8] text-[#8a4b2a]"
    case "processing":
      return "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]"
    default:
      return "bg-black/5 text-black/50"
  }
}

function initials(name: string) {
  return name
    .split(" ")
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
    .toUpperCase()
}

export function NeedsReview({
  items,
}: {
  items: NeedsReviewItem[]
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Needs Review
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Incoming website inquiries that still need attention —
          unresolved or flagged for a decision.
        </p>
      </div>

      <div className="mb-6 flex items-center justify-between border-b border-[var(--portal-border)] pb-5">
        <p className="text-xs font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
          Review Queue
        </p>

        <p className="text-xs font-light text-black/40">
          {items.length} {items.length === 1 ? "item" : "items"}
        </p>
      </div>

      {items.length === 0 ? (
        <div className="rounded-sm border border-[var(--portal-border)] bg-white p-10">
          <p className="font-serif text-2xl font-light">
            Nothing needs review right now.
          </p>

          <p className="mt-2 text-sm font-light text-black/40">
            New website inquiries will appear here as they arrive.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {items.map((item) => (
            <article
              key={item.id}
              className="rounded-sm border border-[var(--portal-border)] bg-white"
            >
              <div className="flex flex-wrap items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5">
                <div className="flex items-start gap-4">
                  <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] font-serif text-sm font-light text-[var(--portal-navy-soft)]">
                    {initials(item.displayName)}
                  </div>

                  <div>
                    <div className="text-sm font-medium">
                      {item.displayName}
                    </div>

                    <div className="mt-0.5 text-xs font-light text-black/45">
                      {item.email}
                    </div>
                  </div>
                </div>

                <div className="flex flex-col items-end gap-2">
                  <span
                    className={`inline-flex rounded-full px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] ${statusClasses(
                      item.status
                    )}`}
                  >
                    {statusLabel(item.status)}
                  </span>

                  <span className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                    {requestTypeLabel(item.requestType)}
                  </span>
                </div>
              </div>

              <div className="grid gap-4 px-6 py-5 md:grid-cols-[1fr_220px]">
                <div>
                  <p className="text-xs font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                    Message
                  </p>

                  <p className="mt-2 text-sm font-light leading-relaxed text-black/60">
                    {item.message || "No message provided."}
                  </p>
                </div>

                <div className="space-y-3 text-xs font-light">
                  <div>
                    <p className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                      Received
                    </p>

                    <p className="mt-1 text-black/60">
                      {item.receivedAtLabel}
                    </p>
                  </div>

                  <div>
                    <p className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                      Property
                    </p>

                    <p className="mt-1 text-black/60">
                      {item.propertyName
                        ? [item.propertyName, item.propertyLocation]
                            .filter(Boolean)
                            .join(" · ")
                        : "Property context unavailable"}
                    </p>
                  </div>

                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  )
}

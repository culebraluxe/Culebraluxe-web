import { PageHeader } from "@/components/portal/page-header"
import { Panel } from "@/components/portal/panel"
import { NeedsReviewActions } from "@/components/portal/write/needs-review-actions"
import type { NeedsReviewItem } from "@/db/needs-review"

function requestTypeLabel(
  requestType: NeedsReviewItem["requestType"]
) {
  switch (requestType) {
    case "private_viewing":
      return "Private Viewing"
    case "property_information":
      return "Property Information"
    case "general_enquiry":
      return "General Enquiry"
    default:
      return requestType
  }
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
      return "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]"
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
      <PageHeader compact eyebrow="Inbox" title="Needs Review">
        <span className="text-xs font-light text-black/40">
          {items.length} {items.length === 1 ? "item" : "items"}
        </span>
      </PageHeader>

      {items.length === 0 ? (
        <Panel compact heading="Inbox is clear">
          <p className="text-sm font-light text-black/45">
            New website inquiries will appear here as they arrive.
          </p>
        </Panel>
      ) : (
        <div className="space-y-3">
          {items.map((item) => (
            <article
              key={item.id}
              className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]"
            >
              <div className="flex flex-wrap items-start justify-between gap-3 px-4 py-3">
                <div className="flex min-w-0 items-start gap-3">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] text-[11px] font-medium text-[var(--portal-navy-soft)]">
                    {initials(item.displayName)}
                  </div>
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">
                      {item.displayName}
                    </div>
                    <div className="truncate text-xs font-light text-black/45">
                      {item.email}
                      {item.propertyName
                        ? ` · ${[item.propertyName, item.propertyLocation]
                            .filter(Boolean)
                            .join(" · ")}`
                        : ""}
                    </div>
                  </div>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <span
                    className={`inline-flex rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.1em] ${statusClasses(
                      item.status
                    )}`}
                  >
                    {statusLabel(item.status)}
                  </span>
                  <span className="text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                    {requestTypeLabel(item.requestType)}
                  </span>
                  <span className="text-xs font-light text-black/40">
                    {item.receivedAtLabel}
                  </span>
                </div>
              </div>

              <p className="px-4 pb-3 text-sm font-light leading-6 text-black/60">
                {item.message || "No message provided."}
              </p>

              <div className="px-4 pb-3">
                <NeedsReviewActions submissionId={item.id} />
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  )
}

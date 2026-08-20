import Link from "next/link"

import type {
  DealParticipant,
  DealWorkspace,
} from "@/db/deal-workspace"
import { CreateTaskForm, TaskActions } from "@/components/portal/write/task-actions"
import { CreateShowingForm, ShowingActions } from "@/components/portal/write/showing-actions"
import { OfferActions, OfferForm } from "@/components/portal/write/offer-actions"
import {
  AddOtherParticipantForm,
  OtherParticipantActions,
} from "@/components/portal/write/participant-actions"

function stageLabel(stage: string) {
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
    default:
      return stage
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
      return "Manual Entry"
    case "whatsapp":
      return "WhatsApp"
    default:
      return channel.charAt(0).toUpperCase() + channel.slice(1)
  }
}

function formatCurrency(value?: number | null) {
  if (!value) return "—"

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value)
}

function participantRole(participant: DealParticipant): string {
  const base =
    participant.roleCategory === "client"
      ? "Client"
      : participant.roleCategory === "owner"
        ? "Owner"
        : participant.roleCategory === "seller"
          ? "Seller"
          : "Other"
  return participant.roleLabel ?? base
}

function offerStatus(status: string): string {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function showingStatus(status: string): string {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function Detail({
  label,
  value,
}: {
  label: string
  value: string
}) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">
        {label}
      </div>
      <div className="mt-2 text-sm font-light leading-6 text-black/70">{value}</div>
    </div>
  )
}

function Empty({ text }: { text: string }) {
  return <div className="px-6 py-10 text-sm font-light text-black/40">{text}</div>
}

export function DealWorkspace({
  workspace,
}: {
  workspace: DealWorkspace
}) {
  const deal = workspace.deal

  if (!deal) {
    return (
      <div>
        <div className="mb-8">
          <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
            Portfolio
          </p>
          <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
            Deal Workspace
          </h1>
        </div>
        <div className="rounded-sm border border-[var(--portal-border)] bg-white p-10">
          <p className="text-sm font-light text-black/45">Deal not found.</p>
        </div>
      </div>
    )
  }

  const property = workspace.property

  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portfolio
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          {property?.name ?? "Deal"}
        </h1>

        <div className="mt-3 flex flex-wrap items-center gap-3">
          <span className="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
            {stageLabel(deal.stage)}
          </span>
          {property?.location && (
            <span className="text-sm font-light text-black/45">
              {property.location}
            </span>
          )}
        </div>

        <Link
          href="/portal/deals"
          className="mt-5 inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
        >
          <span aria-hidden>←</span> Back to Deals Portfolio
        </Link>
      </div>

      <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6 lg:p-8">
        <div className="grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <Detail label="List Price" value={formatCurrency(deal.listPrice)} />
          <Detail label="Offer Price" value={formatCurrency(deal.offerPrice)} />
          <Detail
            label="Closing"
            value={deal.closingDateLabel ?? "—"}
          />
          <Detail
            label="Status"
            value={
              deal.closedAtLabel ? `Closed ${deal.closedAtLabel}` : stageLabel(deal.stage)
            }
          />
        </div>

        <div className="mt-7 grid gap-6 border-t border-[var(--portal-border)] pt-6 md:grid-cols-2">
          <Detail label="Created" value={deal.createdAtLabel} />
          <Detail label="Last Updated" value={deal.updatedAtLabel} />
        </div>
      </section>

      <div className="mt-6 grid gap-6 2xl:grid-cols-3">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">Property</h2>
          </div>
          <div className="px-6 py-5">
            <div className="font-serif text-xl font-light">{property?.name ?? "—"}</div>
            <div className="mt-2 space-y-2 text-xs font-light text-black/45">
              <div>{property?.location ?? "—"}</div>
              <div>
                {[
                  property?.propertyType,
                  property?.bedrooms != null ? `${property.bedrooms} bed` : null,
                  property?.bathrooms != null ? `${property.bathrooms} bath` : null,
                  property?.squareFeet != null
                    ? `${property.squareFeet.toLocaleString("en-US")} SF`
                    : null,
                ]
                  .filter(Boolean)
                  .join(" · ") || "No details on file"}
              </div>
            </div>
          </div>
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">Client</h2>
          </div>
          {workspace.client ? (
            <div className="px-6 py-5">
              <Link
                href={`/portal/clients/${workspace.client.id}`}
                className="inline-flex min-h-11 items-center font-serif text-xl font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
              >
                {workspace.client.displayName}
              </Link>
              <div className="mt-2 space-y-2 text-xs font-light text-black/45">
                {workspace.client.email && <div>{workspace.client.email}</div>}
                {workspace.client.phone && <div>{workspace.client.phone}</div>}
                {!workspace.client.email && !workspace.client.phone && (
                  <div>No contact details on file</div>
                )}
              </div>
            </div>
          ) : (
            <Empty text="No client on record." />
          )}
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">Participants</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Canonical participant roles from the deal.
            </p>
          </div>
          {workspace.participants.length > 0 ? (
            <div>
              {workspace.participants.map((participant) => (
                <div
                  key={participant.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                        {participantRole(participant)}
                      </div>
                      <div className="mt-1 font-serif text-lg font-light">
                        {participant.name}
                      </div>
                      {participant.roleLabel && (
                        <div className="mt-1 text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                          {participant.roleCategory}
                        </div>
                      )}
                    </div>
                    <div className="text-right text-xs font-light text-black/40">
                      {participant.detail && <div>{participant.detail}</div>}
                      <div className="mt-1 uppercase tracking-[0.12em]">
                        {participant.kind}
                      </div>
                    </div>
                  </div>
                  <div className="mt-2">
                    <OtherParticipantActions
                      participantId={participant.id}
                      roleCategory={participant.roleCategory}
                    />
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="No participants on record." />
          )}
          <AddOtherParticipantForm dealId={deal.id} />
        </section>
      </div>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <h2 className="font-serif text-2xl font-light">Open Tasks</h2>
              <p className="mt-1 text-xs font-light text-black/40">
                Milestones and follow-ups on this deal.
              </p>
            </div>
            <span className="text-xs font-light text-black/35">
              {workspace.openTasks.length}
            </span>
          </div>
          {workspace.openTasks.length > 0 ? (
            <div>
              {workspace.openTasks.map((task) => (
                <div
                  key={task.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="font-serif text-lg font-light">{task.title}</div>
                    {task.isOverdue && (
                      <span className="rounded-full bg-[#f3e3d8] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[#8a4b2a]">
                        Overdue
                      </span>
                    )}
                  </div>
                  {task.detail && (
                    <p className="mt-1 text-sm font-light text-black/50">{task.detail}</p>
                  )}
                  <div className="mt-2 text-xs font-light text-black/40">
                    {task.dueAtLabel ?? "Unscheduled"}
                  </div>
                  <div className="mt-3">
                    <TaskActions taskId={task.id} />
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="No open tasks on this deal." />
          )}
          <div className="border-t border-[var(--portal-border)] px-6 py-5">
            <CreateTaskForm
              dealId={deal.id}
              personId={workspace.client?.id}
              compact
            />
          </div>
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <h2 className="font-serif text-2xl font-light">Recent Deal Activity</h2>
              <p className="mt-1 text-xs font-light text-black/40">
                Interactions tied to this deal.
              </p>
            </div>
            <span className="text-xs font-light text-black/35">
              {workspace.activity.length}
            </span>
          </div>
          {workspace.activity.length > 0 ? (
            <div>
              {workspace.activity.map((item) => (
                <div
                  key={item.id}
                  className="grid gap-3 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0 md:grid-cols-[150px_120px_1fr]"
                >
                  <div className="text-xs font-light text-black/40">
                    {item.occurredAtLabel}
                  </div>
                  <div className="text-xs font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                    {channelLabel(item.channel)}
                    {item.direction && (
                      <span className="ml-2 normal-case text-black/35">
                        {item.direction}
                      </span>
                    )}
                  </div>
                  <div>
                    {item.personId ? (
                      <Link
                        href={`/portal/clients/${item.personId}`}
                        className="inline-flex min-h-11 items-center text-sm font-medium text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                      >
                        {item.personName ?? "—"}
                      </Link>
                    ) : (
                      <div className="text-sm font-medium">{item.personName ?? "—"}</div>
                    )}
                    <div className="mt-1 text-sm font-light text-black/55">
                      {item.summary ?? item.title ?? "Interaction"}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Empty text="No deal activity yet." />
          )}
        </section>
      </div>

      <div className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">Offers</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Offer history and counter lineage for this deal.
            </p>
          </div>
          <span className="text-xs font-light text-black/35">
            {workspace.offers.length}
          </span>
        </div>
        {workspace.offers.length > 0 ? (
          <div>
            {workspace.offers.map((offer) => (
              <div
                key={offer.id}
                className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
              >
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-serif text-2xl font-light">
                      {formatCurrency(offer.amount)}
                    </span>
                    <span className="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                      {offerStatus(offer.status)}
                    </span>
                    <span className="rounded-full border border-[var(--portal-border)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-black/50">
                      {offer.isCounter ? "Counter" : "Original"}
                    </span>
                  </div>
                  <div className="mt-2 text-xs font-light text-black/45">
                    {offer.personName && <span>{offer.personName}</span>}
                    <span> · Submitted {offer.submittedAtLabel}</span>
                    {offer.respondedAtLabel && (
                      <span> · Responded {offer.respondedAtLabel}</span>
                    )}
                  </div>
                  {offer.note && (
                    <p className="mt-2 text-sm font-light leading-6 text-black/55">
                      {offer.note}
                    </p>
                  )}
                  <div className="mt-3">
                    <OfferActions offerId={offer.id} status={offer.status} />
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <Empty text="No offers on record for this deal." />
        )}
        <div className="border-t border-[var(--portal-border)] px-6 py-5">
          {(() => {
            const client = workspace.client
            if (!client) {
              return (
                <p className="text-sm font-light text-black/45">
                  Add an offer once a client is linked to this deal.
                </p>
              )
            }
            return (
              <div className="space-y-4">
                <OfferForm dealId={deal.id} personId={client.id} />
                {workspace.offers
                  .filter((offer) => offer.status === 'submitted')
                  .map((offer) => (
                    <OfferForm
                      key={offer.id}
                      dealId={deal.id}
                      personId={client.id}
                      parentOfferId={offer.id}
                      label={`Counter ${formatCurrency(offer.amount)}`}
                    />
                  ))}
              </div>
            )
          })()}
        </div>
      </div>

      <div className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">Showings</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Showing history for this deal.
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-4">
            <span className="text-xs font-light text-black/35">
              {workspace.showings.length}
            </span>
            <Link
              href="/portal/showings"
              className="inline-flex min-h-11 items-center gap-1.5 text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
            >
              View all showings
              <span aria-hidden>→</span>
            </Link>
          </div>
        </div>
        {workspace.showings.length > 0 ? (
          <div>
            {workspace.showings.map((showing) => (
              <div
                key={showing.id}
                className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
              >
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <Link
                        href={`/portal/clients/${showing.personId}`}
                        className="inline-flex min-h-11 items-center font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                      >
                        {showing.personName}
                      </Link>
                      <span className="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                        {showingStatus(showing.status)}
                      </span>
                    </div>
                    <div className="mt-1 text-xs font-light text-black/45">
                      Requested {showing.requestedAtLabel}
                      {showing.scheduledAtLabel &&
                        ` · Scheduled ${showing.scheduledAtLabel}`}
                      {showing.completedAtLabel &&
                        ` · Completed ${showing.completedAtLabel}`}
                      {showing.cancelledAtLabel &&
                        ` · Cancelled ${showing.cancelledAtLabel}`}
                    </div>
                    {showing.feedback && (
                      <p className="mt-2 text-sm font-light leading-6 text-black/55">
                        {showing.feedback}
                      </p>
                    )}
                    <div className="mt-3">
                      <ShowingActions
                        showingId={showing.id}
                        status={showing.status}
                      />
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <Empty text="No showings on record for this deal." />
        )}
        <div className="border-t border-[var(--portal-border)] px-6 py-5">
          {workspace.client && (
            <CreateShowingForm
              personId={workspace.client.id}
              dealId={deal.id}
            />
          )}
        </div>
      </div>

      {deal.notes && (
        <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
          <h2 className="font-serif text-2xl font-light">Deal Notes</h2>
          <p className="mt-4 text-sm font-light leading-7 text-black/55">{deal.notes}</p>
        </section>
      )}
    </div>
  )
}

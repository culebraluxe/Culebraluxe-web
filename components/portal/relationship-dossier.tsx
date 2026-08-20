import Link from "next/link"

import { InteractionLogForm } from "@/components/portal/write/interaction-log"
import { PersonActions } from "@/components/portal/write/person-actions"
import { CreateTaskForm, TaskActions } from "@/components/portal/write/task-actions"
import type {
  RelationshipDossier,
} from "@/db/dossier"

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

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

export function RelationshipDossier({
  dossier,
}: {
  dossier: RelationshipDossier
}) {
  const person = dossier.person

  if (!person) {
    return (
      <div>
        <div className="mb-8">
          <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
            Relationships
          </p>

          <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
            Relationship Dossier
          </h1>
        </div>

        <div className="rounded-sm border border-[var(--portal-border)] bg-white p-10">
          <p className="text-sm font-light text-black/45">
            Person not found.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Relationships
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          {person.displayName}
        </h1>

        <div className="mt-2 flex flex-wrap items-center gap-3 text-sm font-light text-black/45">
          <span>{roleLabel(person.role)}</span>
          <span>·</span>
          <span>{statusLabel(person.status)}</span>
          {person.location && (
            <>
              <span>·</span>
              <span>{person.location}</span>
            </>
          )}
        </div>

        <Link
          href="/portal/clients"
          className="mt-5 inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
        >
          <span aria-hidden>←</span> Back to Client Manager
        </Link>
      </div>

      <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6 lg:p-8">
        <div className="grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <Detail
            label="Assigned Agent"
            value={person.assignedAgent ?? "—"}
          />
          <Detail
            label="Budget"
            value={
              person.budgetMin || person.budgetMax
                ? `${formatCurrency(person.budgetMin)} – ${formatCurrency(
                    person.budgetMax
                  )}`
                : "—"
            }
          />
          <Detail
            label="Timeline"
            value={person.timeline ?? "—"}
          />
          <Detail
            label="Identities"
            value={
              dossier.identities.length > 0
                ? dossier.identities
                    .map((identity) => identity.value)
                    .join(", ")
                : "None on file"
            }
          />
        </div>

        {person.notes && (
          <p className="mt-7 border-t border-[var(--portal-border)] pt-6 text-sm font-light leading-7 text-black/55">
            {person.notes}
          </p>
        )}
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6 lg:p-8">
        <h2 className="font-serif text-2xl font-light">Relationship & Notes</h2>
        <p className="mt-1 text-xs font-light text-black/40">
          Relationship status, client notes, and manual timeline entries.
        </p>
        <div className="mt-6 grid gap-8 lg:grid-cols-2">
          <PersonActions
            personId={person.id}
            initialStatus={person.status}
            initialNotes={person.notes}
          />
          <div>
            <h3 className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
              Log an interaction
            </h3>
            <div className="mt-3">
              <InteractionLogForm personId={person.id} />
            </div>
          </div>
        </div>
      </section>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.9fr)]">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">
              Open Tasks
            </h2>

            <p className="mt-1 text-xs font-light text-black/40">
              Follow-ups and milestones still open for this person.
            </p>
          </div>

          {dossier.openTasks.length > 0 ? (
            <div>
              {dossier.openTasks.map((task) => (
                <div
                  key={task.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="font-serif text-lg font-light">
                      {task.title}
                    </div>

                    {task.isOverdue && (
                      <span className="rounded-full bg-[#f3e3d8] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[#8a4b2a]">
                        Overdue
                      </span>
                    )}
                  </div>

                  {task.detail && (
                    <p className="mt-1 text-sm font-light text-black/50">
                      {task.detail}
                    </p>
                  )}

                  <div className="mt-2 text-xs font-light text-black/40">
                    {task.dueAtLabel ?? "Unscheduled"}
                    {task.propertyName && ` · ${task.propertyName}`}
                    {task.dealPropertyName && ` · ${task.dealPropertyName}`}
                  </div>
                  <div className="mt-3">
                    <TaskActions taskId={task.id} />
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="px-6 py-10 text-sm font-light text-black/40">
              No open tasks for this person.
            </div>
          )}
          <div className="border-t border-[var(--portal-border)] px-6 py-5">
            <CreateTaskForm personId={person.id} compact />
          </div>
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">
              Deals
            </h2>

            <p className="mt-1 text-xs font-light text-black/40">
              Opportunities where this person is the client.
            </p>
          </div>

          {dossier.deals.length > 0 ? (
            <div>
              {dossier.deals.map((deal) => (
                <div
                  key={deal.id}
                  className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div className="flex items-start justify-between gap-4">
                    <Link
                      href={`/portal/deals/${deal.id}`}
                      className="inline-flex min-h-11 items-center font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {deal.propertyName}
                    </Link>

                    <span className="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                      {stageLabel(deal.stage)}
                    </span>
                  </div>

                  <div className="mt-1 text-xs font-light text-black/45">
                    {deal.propertyLocation}
                    {deal.ownerName && ` · Owner: ${deal.ownerName}`}
                    {deal.closingDateLabel && ` · Closes ${deal.closingDateLabel}`}
                  </div>

                  <div className="mt-3 text-sm font-light text-[var(--portal-navy)]">
                    {formatCurrency(deal.offerPrice ?? deal.listPrice)}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="px-6 py-10 text-sm font-light text-black/40">
              No deals for this person yet.
            </div>
          )}
        </section>
      </div>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.9fr)]">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">
              Interaction Timeline
            </h2>

            <p className="mt-1 text-xs font-light text-black/40">
              Calls, messages, email, meetings, showings and notes.
            </p>
          </div>

          {dossier.interactions.length > 0 ? (
            <div>
              {dossier.interactions.map((interaction) => (
                <div
                  key={interaction.id}
                  className="grid gap-3 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0 md:grid-cols-[150px_120px_1fr]"
                >
                  <div className="text-xs font-light text-black/40">
                    {interaction.occurredAtLabel}
                  </div>

                  <div className="text-xs font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                    {channelLabel(interaction.channel)}
                  </div>

                  <div>
                    <div className="text-sm font-medium">
                      {interaction.title ?? "Interaction"}
                    </div>

                    {interaction.summary && (
                      <p className="mt-1 text-sm font-light leading-6 text-black/50">
                        {interaction.summary}
                      </p>
                    )}

                    {(interaction.propertyName ||
                      interaction.dealPropertyName) && (
                      <p className="mt-1 text-xs font-light text-black/40">
                        {[interaction.propertyName, interaction.dealPropertyName]
                          .filter(Boolean)
                          .join(" · ")}
                      </p>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="px-6 py-10 text-sm font-light text-black/40">
              No interaction history yet.
            </div>
          )}
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-white">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">
              Property Interests
            </h2>

            <p className="mt-1 text-xs font-light text-black/40">
              Properties connected to this person.
            </p>
          </div>

          {dossier.interests.length > 0 ? (
            <div>
              {dossier.interests.map((interest) => (
                <div
                  key={interest.id}
                  className="flex items-start justify-between gap-4 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
                >
                  <div>
                    <Link
                      href={`/portal/property-admin/${interest.propertyId}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
                    >
                      {interest.propertyName}
                    </Link>

                    <div className="mt-1 text-xs font-light text-black/45">
                      {interest.location}
                      {interest.price != null && ` · ${formatCurrency(interest.price)}`}
                    </div>
                  </div>

                  <span className="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                    {interest.status.replace(/_/g, " ")}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <div className="px-6 py-10 text-sm font-light text-black/40">
              No property interests yet.
            </div>
          )}
        </section>
      </div>

      <div className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">Showings</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Properties this person has requested or viewed.
            </p>
          </div>
          <span className="text-xs font-light text-black/35">
            {dossier.showings.length}
          </span>
        </div>
        {dossier.showings.length > 0 ? (
          <div>
            {dossier.showings.map((showing) => (
              <div
                key={showing.id}
                className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-serif text-lg font-light">
                        {showing.propertyName ?? showing.dealPropertyName ?? "—"}
                      </span>
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
                    </div>
                    {showing.feedback && (
                      <p className="mt-2 text-sm font-light leading-6 text-black/55">
                        {showing.feedback}
                      </p>
                    )}
                  </div>
                  {showing.dealId && (
                    <Link
                      href={`/portal/deals/${showing.dealId}`}
                      className="shrink-0 text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                    >
                      View deal
                    </Link>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="px-6 py-10 text-sm font-light text-black/40">
            No showings on record.
          </div>
        )}
      </div>

      <div className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">Offers</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Offer history for this person.
            </p>
          </div>
          <span className="text-xs font-light text-black/35">
            {dossier.offers.length}
          </span>
        </div>
        {dossier.offers.length > 0 ? (
          <div>
            {dossier.offers.map((offer) => (
              <div
                key={offer.id}
                className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0"
              >
                <div className="flex flex-wrap items-start justify-between gap-4">
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
                      {offer.dealPropertyName}
                      <span> · Submitted {offer.submittedAtLabel}</span>
                      {offer.respondedAtLabel &&
                        ` · Responded ${offer.respondedAtLabel}`}
                    </div>
                  </div>
                  <Link
                    href={`/portal/deals/${offer.dealId}`}
                    className="shrink-0 text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                  >
                    View deal
                  </Link>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="px-6 py-10 text-sm font-light text-black/40">
            No offers on record.
          </div>
        )}
      </div>

      <div className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">
          Relationship Notes
        </h2>

        <p className="mt-4 text-sm font-light leading-7 text-black/55">
          {person.notes ?? "No notes yet."}
        </p>
      </div>
    </div>
  )
}

function showingStatus(status: string): string {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function offerStatus(status: string): string {
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

      <div className="mt-2 text-sm font-light leading-6 text-black/70">
        {value}
      </div>
    </div>
  )
}

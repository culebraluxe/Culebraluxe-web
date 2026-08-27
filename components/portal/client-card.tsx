"use client"

import Link from "next/link"
import { Mail } from "lucide-react"

import type { Client } from "@/lib/portal/types"
import type { ClientEditorAgent } from "@/components/portal/client-editor"
import { ClientEditor } from "@/components/portal/client-editor"
import { Panel } from "@/components/portal/panel"
import {
  formatCurrency,
  formatPhone,
  ghostBtn,
  initials,
  roleLabel,
  statusDot,
  statusLabel,
} from "@/components/portal/client-display"

// ---------------------------------------------------------------------------
// CLIENTS — Client Card (right column of the Clients working pane).
//
// An elegant, read-mostly digital relationship card for the selected client —
// NOT an editable CRUD form and NOT a manual activity logger. Identity/context
// at a glance (name, role, status, phone, email, location, agent, budget,
// timeline, current next action) plus compact EMAIL / Dossier / Edit actions.
// The card is a peer of the navy Contact History: it sets the shared row height
// (min ~22rem) so both panels stay equal and the history scrolls internally.
// ---------------------------------------------------------------------------

function CardDetail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
        {label}
      </div>
      <div className="mt-1 break-words text-sm font-light leading-5 text-black/70">
        {value}
      </div>
    </div>
  )
}

function evidenceDate(value: string | null | undefined) {
  if (!value) return "Not available"
  const date = new Date(value)
  return Number.isNaN(date.getTime())
    ? "Not available"
    : date.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })
}

export function ClientCard({
  client,
  agents,
  showEdit,
  setShowEdit,
}: {
  client: Client
  agents: ClientEditorAgent[]
  showEdit: boolean
  setShowEdit: (value: boolean) => void
}) {
  const emailHref = client.email ? `mailto:${client.email}` : undefined

  return (
    <Panel
      compact
      heading="Client Card"
      className="flex min-h-[22rem] flex-col"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] font-serif text-lg font-light text-[var(--portal-navy-soft)]">
            {initials(client.displayName)}
          </div>
          <div className="min-w-0">
            <h3 className="truncate font-serif text-xl font-light text-[var(--portal-navy)]">
              {client.displayName}
            </h3>
            <div className="mt-1 flex flex-wrap items-center gap-2">
              <span className="text-xs font-light text-black/50">
                {roleLabel(client.role)}
              </span>
              <span className="inline-flex items-center gap-1.5 rounded-full bg-white/60 px-2 py-0.5 text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                <span
                  aria-hidden
                  className={`h-1.5 w-1.5 rounded-full ${statusDot(client.status)}`}
                />
                {statusLabel(client.status)}
              </span>
            </div>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {emailHref ? (
            <Link href={emailHref} className={ghostBtn}>
              <Mail className="mr-1.5 h-3.5 w-3.5" aria-hidden />
              Email
            </Link>
          ) : null}
          <Link href={`/portal/clients/${client.id}`} className={ghostBtn}>
            Dossier
          </Link>
          <button
            type="button"
            onClick={() => setShowEdit(!showEdit)}
            className={ghostBtn}
          >
            {showEdit ? "Close" : "Edit"}
          </button>
        </div>
      </div>

      <div className="mt-4 grid flex-1 gap-3 border-t border-[var(--portal-panel-border)] pt-3 sm:grid-cols-2">
        <CardDetail label="Phone" value={formatPhone(client.phone) ?? "—"} />
        <CardDetail label="Email" value={client.email ?? "—"} />
        <CardDetail label="Location" value={client.location ?? "—"} />
        <CardDetail label="Agent" value={client.assignedAgent ?? "—"} />
        <CardDetail
          label="Budget"
          value={
            client.budgetMin || client.budgetMax
              ? `${formatCurrency(client.budgetMin)} – ${formatCurrency(
                  client.budgetMax
                )}`
              : "—"
          }
        />
        <CardDetail label="Timeline" value={client.timeline ?? "—"} />
        <CardDetail
          label="Observed communications"
          value={(client.relationshipActivity?.observedCommunicationCount ?? 0).toLocaleString()}
        />
        <CardDetail
          label="Relationship signal"
          value={client.relationshipActivity?.twoWay ? "Two-way" : "—"}
        />
        <CardDetail
          label="Last observed"
          value={evidenceDate(client.relationshipActivity?.lastObservedAt)}
        />
      </div>

      {client.nextAction ? (
        <div className="mt-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-3 py-2">
          <span className="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
            Next action ·{" "}
          </span>
          <span className="font-serif text-base text-[var(--portal-navy)]">
            {client.nextAction.title}
          </span>
          {client.nextAction.occurredAt ? (
            <span className="ml-2 text-xs font-light text-black/45">
              {client.nextAction.occurredAt}
            </span>
          ) : null}
        </div>
      ) : null}

      {showEdit ? (
        <div className="mt-4 border-t border-[var(--portal-panel-border)] pt-4">
          <ClientEditor
            mode="edit"
            client={client}
            agents={agents}
            onSaved={() => setShowEdit(false)}
            onCancel={() => setShowEdit(false)}
          />
        </div>
      ) : null}
    </Panel>
  )
}

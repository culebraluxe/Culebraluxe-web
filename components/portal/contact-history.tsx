"use client"

import { useCallback, useEffect, useState } from "react"
import {
  Calendar,
  Globe,
  Home,
  Mail,
  MessageSquare,
  Phone,
  StickyNote,
  type LucideIcon,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import type { ContactHistoryItem, ContactHistoryResult } from "@/db/contact-history"
import type { RelationshipActivity } from "@/lib/portal/types"

// ---------------------------------------------------------------------------
// CLIENTS — Contact History pane (navy right column of the Client working pane).
//
// Server-side paginated over the canonical `interaction` table (~20/page,
// SQL ORDER BY occurred_at DESC + LIMIT/OFFSET). Newest-first rows show
// channel, date/time, direction (inbound/outbound) and a short subject /
// snippet. The list fills the shared Client Card row and scrolls INSIDE the
// navy pane (overflow-auto) so a long history never grows taller than the
// Client Card; the page resets to 1 when another client is selected. No raw
// L/ODS tables are read by the component.
// ---------------------------------------------------------------------------

const PAGE_SIZE = 20

function channelMeta(channel: string): { label: string; Icon: LucideIcon } {
  switch (channel) {
    case "call":
      return { label: "Call", Icon: Phone }
    case "email":
      return { label: "Email", Icon: Mail }
    case "imessage":
      return { label: "iMessage", Icon: MessageSquare }
    case "sms":
      return { label: "SMS", Icon: MessageSquare }
    case "whatsapp":
      return { label: "WhatsApp", Icon: MessageSquare }
    case "meeting":
      return { label: "Meeting", Icon: Calendar }
    case "showing":
      return { label: "Showing", Icon: Home }
    case "note":
      return { label: "Note", Icon: StickyNote }
    default:
      return {
        label: channel.charAt(0).toUpperCase() + channel.slice(1),
        Icon: Globe,
      }
  }
}

const navBtn =
  "inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-white/20 px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white/70 transition hover:border-white/50 hover:text-white disabled:cursor-not-allowed disabled:opacity-35"

export function ContactHistory({
  clientId,
  relationshipActivity,
}: {
  clientId: string
  relationshipActivity?: RelationshipActivity
}) {
  const [page, setPage] = useState(1)
  const [data, setData] = useState<ContactHistoryResult | null>(null)
  const [loading, setLoading] = useState(true)

  const load = useCallback(async (id: string, p: number) => {
    setLoading(true)
    try {
      const res = await fetch(`/api/portal/clients/${id}/history?page=${p}&pageSize=${PAGE_SIZE}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as ContactHistoryResult
      setData(json)
    } catch (err) {
      console.error("Failed to load contact history:", err)
      setData(null)
    } finally {
      setLoading(false)
    }
  }, [])

  // Selecting a different client resets history to page 1.
  useEffect(() => {
    setPage(1)
  }, [clientId])

  useEffect(() => {
    void load(clientId, page)
  }, [clientId, page, load])

  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const rows = data?.rows ?? []
  const observedCommunicationCount =
    relationshipActivity?.observedCommunicationCount ?? 0

  return (
    <Panel
      variant="feature"
      heading="Contact history"
      action={
        <span className="text-xs font-light text-white/50">
          {total.toLocaleString()} detailed
          {observedCommunicationCount > 0
            ? ` · ${observedCommunicationCount.toLocaleString()} observed`
            : ""}
        </span>
      }
      className="flex min-h-0 flex-col"
    >
      <div className="min-h-0 flex-1 overflow-auto">
        {rows.length === 0 ? (
          <div className="px-4 py-10 text-center text-sm font-light text-white/55">
            <p>{loading ? "Loading…" : "No detailed contact history yet."}</p>
            {!loading && observedCommunicationCount > 0 ? (
              <p className="mt-2 text-xs leading-5 text-white/40">
                {observedCommunicationCount.toLocaleString()} aggregate communications
                are linked to this client; individual historical messages were not imported.
              </p>
            ) : null}
          </div>
        ) : (
          <div className="min-w-[34rem]">
            <div className="grid grid-cols-[6rem_8.5rem_3.5rem_1fr] items-center gap-x-3 border-b border-white/10 px-4 py-1.5 text-[9px] font-light uppercase tracking-[0.14em] text-white/40">
              <span>Type</span>
              <span>Date / Time</span>
              <span>Dir</span>
              <span>Summary</span>
            </div>
            {rows.map((row) => <HistoryRow key={row.id} row={row} />)}
          </div>
        )}
      </div>
      <div className="flex items-center justify-between gap-2 border-t border-white/10 px-3 py-2">
        <button
          type="button"
          disabled={page <= 1}
          onClick={() => setPage(page - 1)}
          className={navBtn}
        >
          ← Prev
        </button>
        <span className="text-[10px] font-light uppercase tracking-[0.12em] text-white/50">
          Page {page} / {totalPages}
        </span>
        <button
          type="button"
          disabled={page >= totalPages}
          onClick={() => setPage(page + 1)}
          className={navBtn}
        >
          Next →
        </button>
      </div>
    </Panel>
  )
}

function HistoryRow({ row }: { row: ContactHistoryItem }) {
  const { label, Icon } = channelMeta(row.channel)
  const dir = row.direction === "inbound" ? "In" : row.direction === "outbound" ? "Out" : "—"
  return (
    <div className="grid grid-cols-[6rem_8.5rem_3.5rem_1fr] items-center gap-x-3 border-b border-white/10 px-4 py-2 last:border-b-0">
      <span className="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-[0.1em] text-white/70">
        <Icon className="h-3 w-3 shrink-0 text-white/55" aria-hidden />
        {label}
      </span>
      <span className="text-[11px] font-light text-white/65">{row.occurredAt}</span>
      <span className="text-[10px] font-light uppercase text-white/50">{dir}</span>
      <span className="truncate text-xs font-light text-white/70">
        {row.title ?? row.summary ?? "—"}
      </span>
    </div>
  )
}

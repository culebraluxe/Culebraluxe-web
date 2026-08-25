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

// ---------------------------------------------------------------------------
// CLIENTS — Contact History pane (navy right column of the Client working pane).
//
// Server-side paginated over the canonical `interaction` table (~20/page,
// SQL ORDER BY occurred_at DESC + LIMIT/OFFSET). Newest-first rows show
// channel, date/time, direction (inbound/outbound) and a short subject /
// snippet. The list scrolls inside the pane (bounded max-height) so a long
// history never stretches the page; the page resets to 1 when another client
// is selected. No raw L/ODS tables are read by the component.
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

export function ContactHistory({ clientId }: { clientId: string }) {
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

  return (
    <Panel
      variant="feature"
      heading="Contact history"
      action={<span className="text-xs font-light text-white/50">{total.toLocaleString()}</span>}
      className="min-h-0"
    >
      <div className="max-h-[26rem] overflow-y-auto">
        {rows.length === 0 ? (
          <p className="px-4 py-10 text-center text-sm font-light text-white/55">
            {loading ? "Loading…" : "No contact history yet."}
          </p>
        ) : (
          rows.map((row) => <HistoryRow key={row.id} row={row} />)
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
  return (
    <div className="border-b border-white/10 px-4 py-2.5 last:border-b-0">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Icon className="h-3.5 w-3.5 shrink-0 text-white/60" aria-hidden />
          <span className="text-[10px] font-medium uppercase tracking-[0.12em] text-white/60">
            {label}
          </span>
          {row.direction ? (
            <span className="rounded-full border border-white/20 px-1.5 py-0.5 text-[9px] font-light uppercase tracking-[0.1em] text-white/55">
              {row.direction === "inbound" ? "Inbound" : "Outbound"}
            </span>
          ) : null}
        </div>
        <span className="shrink-0 text-[10px] font-light text-white/45">{row.occurredAt}</span>
      </div>
      {row.title ? <div className="mt-1 truncate text-sm font-medium text-white">{row.title}</div> : null}
      {row.summary ? (
        <p className="mt-0.5 line-clamp-2 text-xs font-light leading-5 text-white/60">{row.summary}</p>
      ) : null}
    </div>
  )
}

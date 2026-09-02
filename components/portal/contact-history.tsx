"use client"

import Link from "next/link"
import { useCallback, useEffect, useState } from "react"
import {
  Calendar,
  Globe,
  Home,
  Mail,
  MessageSquare,
  Phone,
  StickyNote,
  Video,
  type LucideIcon,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import type {
  AggregateEvidenceHistoryItem,
  ContactHistoryMoment,
  ContactHistoryResult,
} from "@/db/contact-history"
import type {
  ClientRelationshipChannel,
  RelationshipActivity,
} from "@/lib/portal/types"
import {
  channelLine,
  cleanPreview,
  humanDirection,
  sourceContextMoment,
} from "@/lib/relationship-intel/moment-presentation"

// ---------------------------------------------------------------------------
// CLIENTS — Contact History pane (navy right column of the Client working pane).
//
// A Cloze-style relationship-memory sidebar: a compact aggregate header, six
// dense latest-activity source rows, and a persistent Call / Email / Message /
// More action dock. "View all" opens the server-paginated canonical interaction
// archive (~20/page, newest first). The panel scrolls internally and never grows
// taller than the Client Card. Aggregate relationship evidence powers the header
// and source rows; canonical interactions power latest context and the archive.
// No raw L/ODS tables are read.
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

function CompactRelationshipHeader({
  sourceChannels,
  relationshipActivity,
}: {
  sourceChannels: ClientRelationshipChannel[]
  relationshipActivity?: RelationshipActivity
}) {
  const observed = relationshipActivity?.observedCommunicationCount ?? 0
  const inbound = relationshipActivity?.inboundCount ?? 0
  const outbound = relationshipActivity?.outboundCount ?? 0
  const firstObserved = relationshipActivity?.firstObservedAt ?? null
  const lastInbound = relationshipActivity?.lastInboundAt ?? null
  const lastOutbound = relationshipActivity?.lastOutboundAt ?? null
  const activeSourceCount = SOURCE_SLOTS.filter((slot) =>
    sourceChannels.some(slot.matches),
  ).length

  return (
    <div className="border-b border-white/10 px-4 py-2">
      <p className="truncate text-[11px] font-light text-white/70">
        {observed.toLocaleString()} observed · {inbound.toLocaleString()} inbound ·{" "}
        {outbound.toLocaleString()} outbound
        {relationshipActivity?.twoWay ? " · two-way" : ""}
        {firstObserved ? ` · since ${formatAggDate(firstObserved)}` : ""}
      </p>
      <div className="mt-1 flex flex-wrap gap-x-5 gap-y-0.5 text-[10px] font-light text-white/50">
        <span>Last outbound: {lastOutbound ? formatAggDate(lastOutbound) : "—"}</span>
        <span>Last inbound: {lastInbound ? formatAggDate(lastInbound) : "—"}</span>
      </div>
      <div className="mt-0.5 flex flex-wrap gap-x-5 gap-y-0.5 text-[10px] font-light text-white/50">
        <span>First observed: {firstObserved ? formatAggDate(firstObserved) : "—"}</span>
        <span>
          Active sources: {activeSourceCount} of {SOURCE_SLOTS.length}
        </span>
      </div>
    </div>
  )
}

const dockActionCls =
  "inline-flex min-h-8 flex-1 items-center justify-center gap-1.5 rounded-[var(--portal-tab-radius)] border border-white/15 px-2 text-[10px] font-medium uppercase tracking-[0.12em] text-white/75 transition hover:border-[var(--portal-gold)] hover:text-white disabled:cursor-not-allowed disabled:opacity-35"

function QuickActionDock({
  email,
  phone,
  clientId,
}: {
  email?: string | null
  phone?: string | null
  clientId: string
}) {
  const digits = phone ? phone.replace(/[^\d+]/g, "") : null
  const tel = digits ? `tel:${digits}` : null
  const sms = digits ? `sms:${digits}` : null
  const mail = email ? `mailto:${email}` : null
  return (
    <div className="grid grid-cols-4 gap-2 border-t border-white/10 px-3 py-2">
      {tel ? (
        <a href={tel} className={dockActionCls}>
          <Phone className="h-3.5 w-3.5" aria-hidden />
          Call
        </a>
      ) : (
        <button type="button" disabled title="No phone on file" className={dockActionCls}>
          <Phone className="h-3.5 w-3.5" aria-hidden />
          Call
        </button>
      )}
      {mail ? (
        <a href={mail} className={dockActionCls}>
          <Mail className="h-3.5 w-3.5" aria-hidden />
          Email
        </a>
      ) : (
        <button type="button" disabled title="No email on file" className={dockActionCls}>
          <Mail className="h-3.5 w-3.5" aria-hidden />
          Email
        </button>
      )}
      {sms ? (
        <a href={sms} className={dockActionCls}>
          <MessageSquare className="h-3.5 w-3.5" aria-hidden />
          Message
        </a>
      ) : (
        <button type="button" disabled title="No phone on file" className={dockActionCls}>
          <MessageSquare className="h-3.5 w-3.5" aria-hidden />
          Message
        </button>
      )}
      <Link href={`/portal/clients/${clientId}`} className={dockActionCls}>
        <Globe className="h-3.5 w-3.5" aria-hidden />
        More
      </Link>
    </div>
  )
}

export function ContactHistory({
  clientId,
  clientName,
  relationshipActivity,
  email,
  phone,
}: {
  clientId: string
  clientName: string
  relationshipActivity?: RelationshipActivity
  email?: string | null
  phone?: string | null
}) {
  const [page, setPage] = useState(1)
  const [viewAll, setViewAll] = useState(false)
  const [data, setData] = useState<ContactHistoryResult | null>(null)
  const [sourceChannels, setSourceChannels] = useState<ClientRelationshipChannel[]>([])
  const [loadingHistory, setLoadingHistory] = useState(true)
  const [loadingSources, setLoadingSources] = useState(true)

  const loadChannels = useCallback(async (id: string) => {
    setLoadingSources(true)
    try {
      const res = await fetch(`/api/portal/clients/${id}/relationship-channels`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as { channels: ClientRelationshipChannel[] }
      setSourceChannels(json.channels ?? [])
    } catch (err) {
      console.error("Failed to load relationship channels:", err)
      setSourceChannels([])
    } finally {
      setLoadingSources(false)
    }
  }, [])

  const load = useCallback(async (id: string, p: number, recent: boolean) => {
    setLoadingHistory(true)
    try {
      const url = `/api/portal/clients/${id}/history?page=${p}&pageSize=${PAGE_SIZE}${recent ? "&recent=true" : ""}`
      const res = await fetch(url)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as ContactHistoryResult
      setData(json)
    } catch (err) {
      console.error("Failed to load contact history:", err)
      setData(null)
    } finally {
      setLoadingHistory(false)
    }
  }, [])

  useEffect(() => {
    setPage(1)
    setViewAll(false)
    setData(null)
    setSourceChannels([])
  }, [clientId])

  useEffect(() => {
    void loadChannels(clientId)
  }, [clientId, loadChannels])

  useEffect(() => {
    void load(clientId, page, !viewAll)
  }, [clientId, page, viewAll, load])

  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const rows = data?.rows ?? []
  const observedCommunicationCount =
    relationshipActivity?.observedCommunicationCount ?? 0
  const connectedSourceCount = SOURCE_SLOTS.filter((slot) =>
    sourceChannels.some(slot.matches),
  ).length

  return (
    <Panel
      variant="feature"
      heading="Contact History"
      action={
        <span className="flex items-center gap-2 text-xs font-light text-white/50">
          <span>{observedCommunicationCount.toLocaleString()} observed</span>
          {total > 0 ? (
            <button
              type="button"
              onClick={() => {
                setPage(1)
                setViewAll((value) => !value)
              }}
              className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] border border-white/20 px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white/75 transition hover:border-[var(--portal-gold)] hover:text-white"
            >
              {viewAll ? "Sources" : "View all"}
            </button>
          ) : null}
        </span>
      }
      className="flex min-h-0 flex-col"
    >
      <CompactRelationshipHeader
        sourceChannels={sourceChannels}
        relationshipActivity={relationshipActivity}
      />
      <div className="min-h-0 flex-1 overflow-auto">
        {viewAll ? (
          rows.length === 0 ? (
            <div className="px-7 py-6">
              <p className="text-sm font-light text-white/60">
                {loadingHistory ? "Loading…" : "No detailed history yet."}
              </p>
            </div>
          ) : (
            <ol className="relative pb-1 before:absolute before:left-2 before:top-1 before:bottom-1 before:w-px before:bg-white/10 before:content-['']">
              {rows.map((row) =>
                row.kind === "aggregate_evidence" ? (
                  <AggregateTimelineItem key={row.id} item={row} clientName={clientName} />
                ) : (
                  <TimelineMoment key={row.id} moment={row} clientName={clientName} />
                ),
              )}
            </ol>
          )
        ) : (
          <ol className="divide-y divide-white/10">
            {SOURCE_SLOTS.map((slot) => (
              <SourceActivityRow
                key={slot.id}
                slot={slot}
                channel={sourceChannels.find(slot.matches)}
                clientName={clientName}
                loading={loadingSources}
              />
            ))}
          </ol>
        )}
      </div>
      {viewAll ? (
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
      ) : (
        <div className="border-t border-white/10 px-3 py-1.5 text-center text-[10px] font-light uppercase tracking-[0.12em] text-white/45">
          {connectedSourceCount} of {SOURCE_SLOTS.length} sources connected
        </div>
      )}
      <QuickActionDock email={email} phone={phone} clientId={clientId} />
    </Panel>
  )
}

function formatMomentDate(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" })
}

function formatMomentTime(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })
}

function TimelineMoment({
  moment,
  clientName,
}: {
  moment: ContactHistoryMoment
  clientName: string
}) {
  const { label, Icon } = channelMeta(moment.channel)
  const isBurst = moment.count > 1
  const stamp = isBurst
    ? `${formatMomentDate(moment.startedAt)} · ${formatMomentTime(
        moment.startedAt,
      )}–${formatMomentTime(moment.endedAt)}`
    : `${formatMomentDate(moment.startedAt)} · ${formatMomentTime(moment.startedAt)}`
  const direction = humanDirection(moment.direction, clientName)
  const line = channelLine(label, isBurst, moment.count)
  const preview = cleanPreview(moment.preview)
  return (
    <li className="relative pl-8 pb-4">
      <span
        aria-hidden
        className="absolute left-[3px] top-1.5 h-2.5 w-2.5 rounded-full bg-[var(--portal-gold)] ring-2 ring-[var(--portal-navy-deep)]"
      />
      <div className="text-[11px] font-light text-white/60">{stamp}</div>
      {direction ? (
        <div className="mt-0.5 text-[10px] font-medium text-white/85">{direction}</div>
      ) : null}
      <div className="mt-0.5 flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white/70">
        <Icon className="h-3.5 w-3.5 shrink-0 text-white/60" aria-hidden />
        {line}
      </div>
      {preview ? (
        <p className="mt-0.5 truncate text-xs font-light text-white/85">{preview}</p>
      ) : null}
    </li>
  )
}

function formatAggDate(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })
}

/** Channel-appropriate plural noun for the observed-count line. */
function channelNoun(channel: string): string {
  switch (channel) {
    case "email":
      return "emails"
    case "imessage":
      return "iMessages"
    case "sms":
    case "whatsapp":
      return "messages"
    default:
      return "communications"
  }
}

type SourceSlot = {
  id: "phone" | "imessage" | "whatsapp" | "gmail" | "facetime" | "calendar"
  label: string
  Icon: LucideIcon
  matches: (channel: ClientRelationshipChannel) => boolean
}

function sourceIncludes(channel: ClientRelationshipChannel, token: string): boolean {
  return channel.source.toLowerCase().includes(token)
}

const SOURCE_SLOTS: SourceSlot[] = [
  {
    id: "phone",
    label: "Phone",
    Icon: Phone,
    matches: (channel) =>
      (channel.channel === "call" || sourceIncludes(channel, "phone") || sourceIncludes(channel, "call")) &&
      !sourceIncludes(channel, "facetime"),
  },
  {
    id: "imessage",
    label: "iMessage",
    Icon: MessageSquare,
    matches: (channel) =>
      channel.channel === "imessage" || sourceIncludes(channel, "apple_messages"),
  },
  {
    id: "whatsapp",
    label: "WhatsApp",
    Icon: MessageSquare,
    matches: (channel) =>
      channel.channel === "whatsapp" || sourceIncludes(channel, "whatsapp"),
  },
  {
    id: "gmail",
    label: "Email",
    Icon: Mail,
    matches: (channel) =>
      sourceIncludes(channel, "gmail") ||
      (channel.channel === "email" && !sourceIncludes(channel, "calendar")),
  },
  {
    id: "facetime",
    label: "FaceTime",
    Icon: Video,
    matches: (channel) =>
      channel.channel === "facetime" || sourceIncludes(channel, "facetime"),
  },
  {
    id: "calendar",
    label: "Apple Calendar",
    Icon: Calendar,
    matches: (channel) =>
      channel.channel === "calendar" ||
      sourceIncludes(channel, "calendar") ||
      sourceIncludes(channel, "eventkit"),
  },
]

function formatSourceTimestamp(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  })
}

function directionLabel(
  direction: "inbound" | "outbound" | null,
  clientName: string,
): string | null {
  if (direction === "outbound") {
    return humanDirection("outbound", clientName)
  }
  if (direction === "inbound") {
    return humanDirection("inbound", clientName)
  }
  return null
}

function SourceActivityRow({
  slot,
  channel,
  clientName,
  loading,
}: {
  slot: SourceSlot
  channel?: ClientRelationshipChannel
  clientName: string
  loading: boolean
}) {
  const context = channel ? sourceContextMoment(channel) : null
  const preview = context?.preview ?? null
  const timestamp = context?.timestamp ?? null
  const direction = context ? directionLabel(context.direction, clientName) : null
  const fallback = channel?.totalCount
    ? `${channel.totalCount.toLocaleString()} observed ${channelNoun(channel.channel)}`
    : null

  return (
    <li className="relative flex min-h-[3.35rem] items-center gap-3 py-2 pl-10 pr-4">
      <span
        aria-hidden
        className={`absolute left-[14px] top-1/2 h-2.5 w-2.5 -translate-y-1/2 rounded-full ring-2 ring-[var(--portal-navy-deep)] ${
          channel ? "bg-[var(--portal-gold)]" : "bg-white/20"
        }`}
      />
      <div className="flex w-[7.25rem] shrink-0 items-center gap-2 text-[10px] font-medium uppercase tracking-[0.12em] text-white/70">
        <slot.Icon className="h-3.5 w-3.5 shrink-0 text-white/55" aria-hidden />
        <span className="truncate">{slot.label}</span>
      </div>
      <div className="min-w-0 flex-1">
        <p className={`truncate text-xs font-light ${channel ? "text-white/90" : "text-white/35"}`}>
          {loading && !channel
            ? "Loading…"
            : preview ?? fallback ?? "No activity connected"}
        </p>
        {channel ? (
          <p className="mt-0.5 truncate text-[10px] font-light text-white/45">
            {direction ?? (channel.twoWay ? "Two-way relationship" : "Activity observed")}
            {preview && channel.totalCount > 0
              ? ` · ${channel.totalCount.toLocaleString()} total`
              : ""}
          </p>
        ) : null}
      </div>
      <time className="w-[6.75rem] shrink-0 text-right text-[10px] font-light text-white/50">
        {timestamp ? formatSourceTimestamp(timestamp) : "—"}
      </time>
    </li>
  )
}

// Aggregate evidence-only communication history (e.g. Gmail aggregate email that
// is counted in Observed Communications but has no detailed canonical events).
// Rendered in the same navy Cloze timeline, clearly marked as evidence history so
// it never reads as a fabricated detailed interaction.
function AggregateTimelineItem({
  item,
  clientName,
}: {
  item: AggregateEvidenceHistoryItem
  clientName: string
}) {
  const { label, Icon } = channelMeta(item.channel)
  const direction =
    item.isTwoWay
      ? humanDirection("two-way", clientName)
      : item.outboundCount > 0 && item.inboundCount === 0
        ? humanDirection("outbound", clientName)
        : item.inboundCount > 0
          ? humanDirection("inbound", clientName)
          : null
  const noun = channelNoun(item.channel)
  const rangeLabel =
    item.firstObservedAt && item.lastObservedAt
      ? `Observed ${formatAggDate(item.firstObservedAt)} – ${formatAggDate(item.lastObservedAt)}`
      : null
  return (
    <li className="relative pl-8 pb-4">
      <span
        aria-hidden
        className="absolute left-[3px] top-1.5 h-2.5 w-2.5 rounded-full bg-[var(--portal-gold)] ring-2 ring-[var(--portal-navy-deep)]"
      />
      <div className="text-[11px] font-light text-white/60">
        {item.lastObservedAt ? formatAggDate(item.lastObservedAt) : "Observed history"}
      </div>
      {direction ? (
        <div className="mt-0.5 text-[10px] font-medium text-white/85">{direction}</div>
      ) : null}
      <div className="mt-0.5 flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white/70">
        <Icon className="h-3.5 w-3.5 shrink-0 text-white/60" aria-hidden />
        {label} HISTORY
      </div>
      <p className="mt-0.5 text-xs font-light text-white/85">
        {item.totalCount} observed {noun}
      </p>
      {item.totalCount > 0 ? (
        <p className="mt-0.5 text-[11px] font-light text-white/60">
          {item.inboundCount} inbound · {item.outboundCount} outbound
        </p>
      ) : null}
      {rangeLabel ? (
        <p className="mt-0.5 text-[11px] font-light text-white/45">{rangeLabel}</p>
      ) : null}
    </li>
  )
}

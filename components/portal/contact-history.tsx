"use client"

import Link from "next/link"
import { useCallback, useEffect, useState } from "react"
import {
  Calendar,
  Globe,
  Mail,
  MessageSquare,
  Phone,
  Video,
  type LucideIcon,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import type {
  ClientRelationshipChannel,
  RelationshipActivity,
} from "@/lib/portal/types"
import {
  humanDirection,
  sourceContextMoment,
} from "@/lib/relationship-intel/moment-presentation"

// ---------------------------------------------------------------------------
// CLIENTS — Contact History pane (navy right column of the Client working pane).
//
// A GLANCE PANEL, not an archive. One compact aggregate header and six source
// rows answering the two questions this screen exists for: when did we last
// speak, and what was the last contact message. Aggregate evidence powers the
// header; the warehouse relationship channels power the rows, each carrying its
// own last-contact time and newest bounded context.
//
// There is deliberately no "View all" and no per-message timeline. A full text
// chain belongs on the phone, not here, and shipping every message into the
// warehouse to feed an archive this panel no longer shows was the most expensive
// thing in the pipeline (Ami alone was 5,519 interaction rows for 4 source rows).
// No raw L/ODS tables are read.
// ---------------------------------------------------------------------------

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
  const [sourceChannels, setSourceChannels] = useState<ClientRelationshipChannel[]>([])
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

  useEffect(() => {
    setSourceChannels([])
  }, [clientId])

  useEffect(() => {
    void loadChannels(clientId)
  }, [clientId, loadChannels])

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
        </span>
      }
      className="flex min-h-0 flex-col"
    >
      <CompactRelationshipHeader
        sourceChannels={sourceChannels}
        relationshipActivity={relationshipActivity}
      />
      <div className="min-h-0 flex-1 overflow-auto">
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
      </div>
      <div className="border-t border-white/10 px-3 py-1.5 text-center text-[10px] font-light uppercase tracking-[0.12em] text-white/45">
        {connectedSourceCount} of {SOURCE_SLOTS.length} sources connected
      </div>
      <QuickActionDock email={email} phone={phone} clientId={clientId} />
    </Panel>
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


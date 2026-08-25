import { Ban, Check, Circle, Clock, type LucideIcon } from "lucide-react"
import type { ReactNode } from "react"

import { cn } from "@/lib/utils"

export type PortalTimelineItem = {
  id: string
  actor: string
  detail?: ReactNode
  icon?: LucideIcon
  text: ReactNode
  timestamp: string
  tone?: "default" | "attention" | "success"
}

const timelineDotTone = {
  default: "bg-[var(--portal-navy)]",
  attention: "bg-[var(--portal-archive)]",
  success: "bg-[var(--portal-success)]",
}

export function ActivityTimeline({ items }: { items: PortalTimelineItem[] }) {
  return (
    <div className="relative">
      <span
        className="absolute bottom-2 left-[5px] top-2 w-px bg-[var(--portal-border)]"
        aria-hidden
      />
      <ol className="space-y-5" aria-label="Recent activity">
        {items.map(({ actor, detail, icon: Icon, id, text, timestamp, tone = "default" }) => (
          <li key={id} className="relative flex gap-4 pl-6">
            <span className="absolute left-0 top-1 flex h-3 w-3 items-center justify-center rounded-full border border-[var(--portal-border)] bg-white">
              <span className={cn("h-1.5 w-1.5 rounded-full", timelineDotTone[tone])} />
            </span>
            <div className="min-w-0">
              <time className="text-xs font-light text-black/40">{timestamp}</time>
              <div className="mt-1 text-sm font-medium text-[var(--portal-navy)]">{actor}</div>
              <div className="mt-0.5 flex items-start gap-1.5 text-sm font-light leading-6 text-black/65">
                {Icon ? <Icon className="mt-1 h-3.5 w-3.5 shrink-0 text-[var(--portal-blue-gray)]" /> : null}
                <span>{text}</span>
              </div>
              {detail ? <div className="mt-1 text-xs font-light text-black/40">{detail}</div> : null}
            </div>
          </li>
        ))}
      </ol>
    </div>
  )
}

export type PortalProcessStep = {
  id: string
  label: string
  note?: ReactNode
  state: "complete" | "current" | "upcoming" | "blocked"
}

const stepTone = {
  complete: "border-[var(--portal-success)] bg-[var(--portal-success)] text-white",
  current: "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white",
  blocked: "border-[var(--portal-archive)] bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]",
  upcoming: "border-[var(--portal-border)] bg-white text-black/30",
}

function StepIcon({ state }: { state: PortalProcessStep["state"] }) {
  if (state === "complete") return <Check className="h-3.5 w-3.5" />
  if (state === "current") return <Clock className="h-3.5 w-3.5" />
  if (state === "blocked") return <Ban className="h-3.5 w-3.5" />
  return <Circle className="h-3 w-3" />
}

export function ProcessSteps({ steps }: { steps: PortalProcessStep[] }) {
  return (
    <ol>
      {steps.map(({ id, label, note, state }, index) => (
        <li key={id} className="relative flex gap-4 pb-6 last:pb-0">
          {index < steps.length - 1 ? (
            <span className="absolute left-[11px] top-6 h-full w-px bg-[var(--portal-border)]" aria-hidden />
          ) : null}
          <span
            className={cn(
              "relative z-10 flex h-6 w-6 shrink-0 items-center justify-center rounded-full border",
              stepTone[state],
            )}
            aria-hidden
          >
            <StepIcon state={state} />
          </span>
          <div className="min-w-0 pt-0.5">
            <div className="text-sm font-medium text-[var(--portal-navy)]">{label}</div>
            {note ? <div className="mt-0.5 text-xs font-light text-black/45">{note}</div> : null}
            <span className="sr-only">Status: {state}</span>
          </div>
        </li>
      ))}
    </ol>
  )
}

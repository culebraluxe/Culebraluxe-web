"use client"

import { useCallback, useEffect, useState } from "react"

import {
  adaptFlightRecorderTransaction,
  type FlightRecorderTrace,
} from "@/lib/flight-recorder-adapter"
import type { FlightRecorderTransaction } from "@/lib/flight-recorder-contract"
import { FlightRecorderPage } from "./flight-recorder-console/FlightRecorderPage"

// FLIGHT-RECORDER-CONSOLE-SHELL — the bridge component for the portal route.
// It loads the canonical Flight Recorder transaction read model (real deal,
// exact workflow instance(s), exact persisted definitions, real trace evidence)
// and adapts it into the console read-model. Runtime Inspector remains a
// separate engineering surface.
export function FlightRecorderConsoleShell({
  instanceId,
  defaultEventId,
}: {
  instanceId: string
  defaultEventId?: string
}) {
  const [trace, setTrace] = useState<FlightRecorderTrace | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    setError(null)
    try {
      const res = await fetch(`/api/portal/flight-recorder/${instanceId}`)
      if (!res.ok) {
        // The route knows WHY it failed; this screen must repeat that reason rather than reduce it
        // to "HTTP 503", which told the operator nothing about a bad id in the link.
        const body = (await res.json().catch(() => null)) as
          | { error?: string; detail?: string }
          | null
        if (res.status === 404) throw new Error("Trace not found for this instance")
        throw new Error(body?.detail ?? body?.error ?? `HTTP ${res.status}`)
      }
      const json = (await res.json()) as FlightRecorderTransaction
      setTrace(adaptFlightRecorderTransaction(json))
    } catch (err) {
      setError((err as Error)?.message ?? "failed to load")
    }
  }, [instanceId])

  useEffect(() => {
    void load()
  }, [load])

  if (error) {
    return (
      <div className="grid h-full place-items-center bg-[#0b1220] text-sm text-slate-300">
        <div className="rounded-lg border border-white/10 px-6 py-4">{error}</div>
      </div>
    )
  }

  if (!trace) {
    return (
      <div className="grid h-full place-items-center bg-[#0b1220] text-sm text-slate-400">
        Loading trace…
      </div>
    )
  }

  return (
    <FlightRecorderPage
      trace={trace}
      defaultEventId={defaultEventId ?? trace.events[0]?.id}
    />
  )
}

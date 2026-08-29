"use client"

import { useCallback, useEffect, useState } from "react"

import type { RuntimeInspection } from "@/lib/runtime-inspector"
import {
  adaptRuntimeInspection,
  type FlightRecorderTrace,
  type ResolvedConsoleContext,
} from "@/lib/flight-recorder-adapter"
import { FlightRecorderPage } from "./flight-recorder-console/FlightRecorderPage"

// FLIGHT-RECORDER-CONSOLE-SHELL — the bridge component for the new parallel
// portal route. It loads the real engine's Runtime Inspector payload from the
// existing API, adapts it into the Flight Recorder console read-model, and
// renders the console. The Runtime Inspector page itself is left untouched.
type RawPayload = {
  inspection: RuntimeInspection
  nodeTypes: Record<string, string>
  businessContext?: ResolvedConsoleContext
}

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
      const res = await fetch(`/api/portal/runtime-inspector/${instanceId}`)
      if (!res.ok) {
        if (res.status === 404) throw new Error("Trace not found for this instance")
        throw new Error(`HTTP ${res.status}`)
      }
      const json = (await res.json()) as RawPayload
      setTrace(
        adaptRuntimeInspection({
          inspection: json.inspection,
          nodeTypes: json.nodeTypes,
          resolvedBusinessContext: json.businessContext,
        }),
      )
    } catch (err) {
      setError((err as Error)?.message ?? "failed to load")
    }
  }, [instanceId])

  useEffect(() => {
    void load()
  }, [load])

  if (error) {
    return (
      <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-300">
        <div className="rounded-lg border border-white/10 px-6 py-4">{error}</div>
      </div>
    )
  }

  if (!trace) {
    return (
      <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-400">
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

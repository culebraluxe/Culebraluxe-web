"use client"

import { useCallback, useEffect, useMemo, useState } from "react"

import { Panel } from "@/components/portal/panel"
import type { RuntimeInspection, NodeRuntimeState } from "@/lib/runtime-inspector"

// WORKFLOW RUNTIME INSPECTOR — expected topology + actual execution evidence
// overlaid as a navy/gold CulebraLuxe business-process debugger.

type Payload = {
  inspection: RuntimeInspection
  nodeLabels: Record<string, string>
  nodeDescriptions: Record<string, string>
}

const STATE_META: Record<NodeRuntimeState, { label: string; cls: string }> = {
  NOT_VISITED: { label: "Not visited", cls: "border-white/10 text-white/35" },
  COMPLETED: { label: "Completed", cls: "border-[var(--portal-gold)]/60 text-white/90" },
  CURRENT: { label: "Current", cls: "border-[var(--portal-gold)] text-white ring-2 ring-[var(--portal-gold)]/40" },
  FAILED: { label: "Failed", cls: "border-red-400/60 text-red-200" },
  RECOVERED: { label: "Recovered", cls: "border-amber-300/60 text-amber-100" },
}

function fmtTime(iso: string | null): string {
  if (!iso) return "—"
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleTimeString("en-US", { hour12: false }) + "." + String(d.getMilliseconds()).padStart(3, "0")
}

function fmtDur(ms: number | null): string {
  if (ms == null) return "—"
  if (ms < 1000) return `${ms} ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)} s`
  return `${(ms / 60000).toFixed(1)} min`
}

function fmtRel(ms: number): string {
  return ms < 1000 ? `+${ms} ms` : `+${(ms / 1000).toFixed(1)} s`
}

export function RuntimeInspector({ instanceId }: { instanceId: string }) {
  const [payload, setPayload] = useState<Payload | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [atIso, setAtIso] = useState<string | null>(null) // null = live
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const [showTimeline, setShowTimeline] = useState(true)

  const load = useCallback(async (at: string | null) => {
    setError(null)
    try {
      const q = at ? `?at=${encodeURIComponent(at)}` : ""
      const res = await fetch(`/api/portal/runtime-inspector/${instanceId}${q}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as Payload
      setPayload(json)
      setSelectedNode((prev) => prev ?? json.inspection.currentNodeId ?? null)
    } catch (err) {
      setError((err as Error)?.message ?? "failed to load")
    }
  }, [instanceId])

  useEffect(() => {
    void load(atIso)
  }, [atIso, load])

  const startMs = useMemo(
    () => (payload ? new Date(payload.inspection.startIso ?? "").getTime() : 0),
    [payload],
  )
  const endMs = useMemo(
    () => (payload ? new Date(payload.inspection.endIso ?? "").getTime() : 0),
    [payload],
  )
  const liveMs = useMemo(() => (payload ? new Date().getTime() : 0), [payload])

  const eva = payload?.inspection.expectedVsActual

  return (
    <Panel variant="feature" heading="Runtime Inspector" className="min-h-0 flex flex-col">
      {error ? (
        <p className="px-5 py-4 text-sm font-light text-red-200">{error}</p>
      ) : !payload ? (
        <p className="px-5 py-4 text-sm font-light text-white/50">Loading trace…</p>
      ) : (
        <div className="min-h-0 flex-1 space-y-4 overflow-auto p-4">
          <div className="text-xs font-light text-white/55">
            Instance <span className="text-white/85">{payload.inspection.workflowInstanceId}</span>
            {payload.inspection.definitionKey ? (
              <>
                {" · "}
                {payload.inspection.definitionKey}
                {payload.inspection.definitionVersion != null
                  ? ` v${payload.inspection.definitionVersion}`
                  : ""}
              </>
            ) : null}
          </div>

          {/* Time machine */}
          <div className="rounded-lg border border-white/10 p-3">
            <div className="flex items-center justify-between text-[10px] uppercase tracking-[0.14em] text-white/45">
              <span>{atIso ? fmtTime(payload.inspection.startIso) : "start"}</span>
              <span className="text-[var(--portal-gold)]">
                {atIso ? `at ${fmtTime(atIso)}` : "live"}
              </span>
              <span>{fmtTime(payload.inspection.endIso)}</span>
            </div>
            <input
              type="range"
              min={startMs}
              max={endMs || liveMs}
              value={atIso ? new Date(atIso).getTime() : endMs || liveMs}
              onChange={(e) =>
                setAtIso(Number(e.target.value) ? new Date(Number(e.target.value)).toISOString() : null)
              }
              className="mt-2 w-full accent-[var(--portal-gold)]"
            />
            <div className="mt-1 flex items-center justify-between">
              <button
                type="button"
                onClick={() => setAtIso(null)}
                className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold)] hover:text-white"
              >
                Live
              </button>
              <span className="text-[10px] font-light uppercase tracking-[0.12em] text-white/40">
                {atIso ? "visual replay at T" : "current state"}
              </span>
            </div>
          </div>

          {/* Expected vs actual */}
          {eva ? (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              {[
                ["Nodes", `${eva.nodesVisited}/${eva.nodesExpected}`],
                ["Transitions", `${eva.transitionsTaken}`],
                ["Unexpected", `${eva.unexpectedTransitions}`, eva.unexpectedTransitions > 0 ? "text-red-300" : ""],
                ["Repeated", `${eva.repeatedNodes}`],
                ["Failed", `${eva.failedEvents}`, eva.failedEvents > 0 ? "text-red-300" : ""],
                ["Recovered", `${eva.recoveredFailures}`, eva.recoveredFailures > 0 ? "text-amber-200" : ""],
                ["Current", payload.inspection.currentNodeId ? (payload.nodeLabels[payload.inspection.currentNodeId] ?? payload.inspection.currentNodeId) : "—"],
                ["Elapsed", fmtDur(eva.workflowElapsedMs)],
              ].map(([k, v, tone]) => (
                <div key={String(k)} className="rounded-lg border border-white/10 px-3 py-2">
                  <div className="text-[9px] uppercase tracking-[0.14em] text-white/40">{k}</div>
                  <div className={`mt-0.5 text-sm font-light ${tone ?? "text-white/85"}`}>{v}</div>
                </div>
              ))}
            </div>
          ) : null}

          {/* Node overlay */}
          <div>
            <div className="mb-1.5 text-[10px] uppercase tracking-[0.14em] text-white/45">
              Topology · runtime overlay
            </div>
            <ol className="space-y-1.5">
              {payload.inspection.nodes.map((n) => {
                const meta = STATE_META[n.state]
                return (
                  <li key={n.nodeId}>
                    <button
                      type="button"
                      onClick={() => setSelectedNode(n.nodeId)}
                      className={`w-full rounded-lg border px-3 py-2 text-left ${meta.cls} ${selectedNode === n.nodeId ? "ring-1 ring-[var(--portal-gold)]" : ""}`}
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-sm font-medium">
                          {payload.nodeLabels[n.nodeId] ?? n.nodeId}
                        </span>
                        <span className="text-[9px] uppercase tracking-[0.12em] text-white/40">
                          {meta.label}
                          {n.executionCount > 1 ? ` · x${n.executionCount}` : ""}
                        </span>
                      </div>
                      {n.state !== "NOT_VISITED" ? (
                        <div className="mt-0.5 text-[10px] font-light text-white/45">
                          {fmtTime(n.enteredAt)} → {fmtTime(n.completedAt)} · {fmtDur(n.durationMs)}
                        </div>
                      ) : null}
                    </button>
                  </li>
                )
              })}
            </ol>
          </div>

          {/* Node inspector */}
          {selectedNode ? (
            <div className="rounded-lg border border-[var(--portal-gold)]/40 p-3">
              <div className="text-[10px] uppercase tracking-[0.14em] text-[var(--portal-gold)]">
                Node inspector
              </div>
              <div className="mt-1 text-sm font-medium text-white">
                {payload.nodeLabels[selectedNode] ?? selectedNode}
              </div>
              <p className="mt-0.5 text-xs font-light leading-5 text-white/55">
                {payload.nodeDescriptions[selectedNode] || "No description."}
              </p>
              {(() => {
                const n = payload.inspection.nodes.find((x) => x.nodeId === selectedNode)
                if (!n) return null
                return (
                  <dl className="mt-2 grid grid-cols-2 gap-x-3 gap-y-1 text-[11px] font-light text-white/60">
                    <dt className="text-white/35">State</dt>
                    <dd>{STATE_META[n.state].label}</dd>
                    <dt className="text-white/35">Entered</dt>
                    <dd>{fmtTime(n.enteredAt)}</dd>
                    <dt className="text-white/35">Completed</dt>
                    <dd>{fmtTime(n.completedAt)}</dd>
                    <dt className="text-white/35">Duration</dt>
                    <dd>{fmtDur(n.durationMs)}</dd>
                    <dt className="text-white/35">Trigger (caused by)</dt>
                    <dd className="truncate">{n.triggerEventId ?? "—"}</dd>
                    <dt className="text-white/35">Executions</dt>
                    <dd>{n.executionCount}</dd>
                  </dl>
                )
              })()}
            </div>
          ) : null}

          {/* Timeline */}
          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <span className="text-[10px] uppercase tracking-[0.14em] text-white/45">Timeline</span>
              <button
                type="button"
                onClick={() => setShowTimeline((v) => !v)}
                className="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold)] hover:text-white"
              >
                {showTimeline ? "hide" : "show"}
              </button>
            </div>
            {showTimeline ? (
              <ol className="max-h-72 space-y-1 overflow-auto pr-1">
                {payload.inspection.timeline.map((e, i) => (
                  <li key={e.id} className="flex gap-2 rounded border-l-2 border-white/10 py-1 pl-2 text-[11px]">
                    <span className="shrink-0 text-white/40">{fmtTime(e.occurredAt)}</span>
                    <span className="shrink-0 text-white/30">{fmtRel(e.relativeMs)}</span>
                    <span className="w-28 shrink-0 font-medium text-[var(--portal-gold)]">{e.eventType}</span>
                    <span className="shrink-0 text-white/40">{e.system}</span>
                    <span className="truncate text-white/70">{e.summary ?? ""}</span>
                  </li>
                ))}
                {payload.inspection.timeline.length === 0 ? (
                  <li className="text-xs font-light text-white/40">No trace events recorded yet.</li>
                ) : null}
              </ol>
            ) : null}
          </div>

          {/* Causality */}
          <div>
            <div className="mb-1.5 text-[10px] uppercase tracking-[0.14em] text-white/45">Causality</div>
            <ol className="space-y-1">
              {payload.inspection.timeline
                .filter((e) => e.causationId)
                .map((e) => (
                  <li key={`c-${e.id}`} className="flex items-center gap-2 text-[11px] font-light text-white/60">
                    <span className="text-white/30">←</span>
                    <span className="truncate text-white/40">{e.causationId}</span>
                    <span className="text-[var(--portal-gold)]">{e.eventType}</span>
                  </li>
                ))}
              {payload.inspection.timeline.filter((e) => e.causationId).length === 0 ? (
                <li className="text-xs font-light text-white/40">No causation links recorded.</li>
              ) : null}
            </ol>
          </div>
        </div>
      )}
    </Panel>
  )
}

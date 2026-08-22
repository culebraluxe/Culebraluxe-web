"use client"

import { useEffect, useMemo, useState } from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock,
  ExternalLink,
  Gauge,
  GitBranch,
  Hourglass,
  Layers,
  Search,
  ShieldAlert,
  TerminalSquare,
  TrendingUp,
  User,
  UserCheck,
  UserX,
} from "lucide-react"

import type { FactorySnapshot } from "@/lib/factory-command-center-data"
import type { WorkflowSummary } from "@/workflow_app/read-service"
import { formatTime, runResultPill, shortId, statePill } from "@/lib/command-console-ui"

// ---------------------------------------------------------------------------
// AI Software Factory Command Center (ENG-16) — the PARENT operating console.
//
// One screen, three information layers (all read projections over the REAL
// control-plane tables — no duplicate state):
//   1. Executive rollup   — how the factory is doing (net-net + workstreams)
//   2. Agent dispatch/capacity — who is assigned/idle/blocked + next eligible
//   3. Dependency-aware pipeline — what work is flowing, what each worker
//      produces (worker + story + evidence together on every card)
//
// Drill-down: every story card opens the Story Execution Cockpit
// (/portal/command-console/[id]); workflow cockpit cards open the engine
// instance cockpit (/portal/workflows/[instanceId]). `focusStoryId` (from the
// ?focus= query param) highlights + scrolls the story the user came back from
// so context is never lost.
// ---------------------------------------------------------------------------

const STAGE_ORDER = ["ready", "active", "blocked", "planned", "hold", "complete"] as const

const STAGE_META: Record<
  (typeof STAGE_ORDER)[number],
  { label: string; hint: string; dot: string }
> = {
  ready: {
    label: "Eligible next",
    hint: "Ready with all dependencies complete — dispatchable now",
    dot: "bg-sky-400",
  },
  active: {
    label: "In progress",
    hint: "Claimed or running — work flowing through the factory",
    dot: "bg-emerald-400",
  },
  blocked: {
    label: "Blocked / waiting",
    hint: "Hard blocked, failed, or waiting on an uncompleted dependency",
    dot: "bg-red-400",
  },
  planned: {
    label: "Planned",
    hint: "Defined on the board, not yet authorized to execute",
    dot: "bg-slate-500",
  },
  hold: {
    label: "Deferred / hold",
    hint: "Deferred or held — paused, not flowing",
    dot: "bg-slate-600",
  },
  complete: {
    label: "Complete",
    hint: "Done — evidence recorded",
    dot: "bg-emerald-400/60",
  },
}

const WORKSTREAM_FILTERS = [
  "All",
  "ENG",
  "CRM",
  "PORTAL",
  "TXN",
  "ADMIN",
  "AUTH",
  "CONTENT",
  "HARDEN",
  "PUBLIC",
] as const

const COLLAPSED_DEFAULT: Record<string, boolean> = {
  ready: false,
  active: false,
  blocked: false,
  planned: true,
  hold: true,
  complete: true,
}

export function FactoryCommandCenter({
  snapshot,
  focusStoryId,
}: {
  snapshot: FactorySnapshot
  focusStoryId: string | null
}) {
  const router = useRouter()
  const [workstream, setWorkstream] = useState<(typeof WORKSTREAM_FILTERS)[number]>("All")
  const [query, setQuery] = useState("")
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(COLLAPSED_DEFAULT)

  // Context preservation: when the user drills back from a story cockpit with
  // ?focus=<storyId>, scroll to and highlight that story card.
  useEffect(() => {
    if (!focusStoryId) return
    const el = document.getElementById(`pipeline-card-${focusStoryId}`)
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" })
      el.classList.add("factory-focus-flash")
      const t = window.setTimeout(() => el.classList.remove("factory-focus-flash"), 2200)
      return () => window.clearTimeout(t)
    }
  }, [focusStoryId])

  if (!snapshot.ready) {
    return (
      <div className="min-h-screen bg-[#0a0f1a] text-slate-200">
        <div className="mx-auto max-w-[1600px] px-5 py-24 text-center">
          <Layers className="mx-auto h-9 w-9 text-[#c6a15b]" />
          <h1 className="mt-5 font-serif text-2xl font-light text-white">
            Factory command center not ready
          </h1>
          <p className="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-slate-400">
            The Story Board control-plane tables are not present. Apply migrations
            021–030 (DEV) or verify the production control-plane before using the
            command center.
          </p>
        </div>
      </div>
    )
  }

  const rollup = snapshot.rollup!
  const capacity = snapshot.capacity
  const activeWorker = capacity.workers.find((w) => w.kind === "assigned") ?? null
  const blockedWorker = capacity.workers.find((w) => w.kind === "blocked") ?? null
  const idle = capacity.workers.find((w) => w.kind === "idle") ?? null

  const workerPill = activeWorker
    ? { label: "Worker busy", cls: "border-emerald-400/40 text-emerald-300" }
    : blockedWorker
      ? { label: "Worker blocked", cls: "border-red-400/50 text-red-300" }
      : { label: "Worker idle", cls: "border-slate-500/40 text-slate-400" }

  const filteredNodes = useMemo(() => {
    const q = query.trim().toLowerCase()
    return snapshot.pipeline.nodes.filter((n) => {
      if (workstream !== "All" && n.workstream !== workstream) return false
      if (q && !n.storyId.toLowerCase().includes(q) && !n.title.toLowerCase().includes(q)) {
        return false
      }
      return true
    })
  }, [snapshot.pipeline.nodes, workstream, query])

  const edgesByFrom = useMemo(() => {
    const map = new Map<string, Map<string, boolean | null>>()
    for (const edge of snapshot.pipeline.edges) {
      // External (untracked) refs have no board story id; they resolve to
      // "external" in the card by lookup miss — never stored under a fake key.
      if (edge.to === null) continue
      const inner = map.get(edge.from) ?? new Map<string, boolean | null>()
      inner.set(edge.to, edge.satisfied)
      map.set(edge.from, inner)
    }
    return map
  }, [snapshot.pipeline.edges])

  const gatedNodes = snapshot.pipeline.nodes.filter((n) => n.gated)
  const gatedStoryIds = new Set(snapshot.pipeline.gatedWork)
  const readyStoryIds = new Set(snapshot.pipeline.readyWork)

  return (
    <div className="min-h-screen bg-[#0a0f1a] text-slate-200">
      {/* Layer 1 — executive health strip */}
      <header className="border-b border-white/10 bg-[#0d1424]">
        <div className="mx-auto flex max-w-[1680px] flex-wrap items-center gap-x-8 gap-y-3 px-5 py-4">
          <div className="flex items-center gap-3">
            <TerminalSquare className="h-5 w-5 text-[#c6a15b]" />
            <h1 className="font-serif text-xl font-light uppercase tracking-[0.14em] text-white">
              Factory Command Center
            </h1>
          </div>

          <div className="flex items-center gap-2 text-xs font-light text-slate-400">
            <span className="inline-block h-2 w-2 rounded-full bg-emerald-400" />
            control plane healthy
          </div>

          <div className={`rounded-full border px-3 py-1 text-[11px] font-light tracking-wide ${workerPill.cls}`}>
            {workerPill.label}
          </div>

          <div className="flex items-center gap-2 text-xs font-light text-slate-300">
            <Activity className="h-3.5 w-3.5 text-slate-500" />
            <span className="tabular-nums">{capacity.assignedCount}</span> assigned
          </div>
          <div className="flex items-center gap-2 text-xs font-light text-slate-300">
            <Clock className="h-3.5 w-3.5 text-slate-500" />
            <span className="tabular-nums">{snapshot.pipeline.readyWork.length}</span> ready work
          </div>
          <div className="flex items-center gap-2 text-xs font-light text-amber-300">
            <Hourglass className="h-3.5 w-3.5 text-amber-400/70" />
            <span className="tabular-nums">{snapshot.humanGateCount}</span> human gates
          </div>
          <div className="flex items-center gap-2 text-xs font-light text-slate-300">
            <TrendingUp className="h-3.5 w-3.5 text-slate-500" />
            <span className="tabular-nums">{rollup.netNet}%</span> net-net
          </div>
        </div>
      </header>

      <div className="mx-auto max-w-[1680px] space-y-5 px-5 py-5">
        {/* Layer 1 detail — executive rollup */}
        <ExecutiveRollup snapshot={snapshot} rollup={rollup} />

        <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
          {/* Layer 3 — dependency-aware factory pipeline */}
          <main className="min-w-0">
            <section className="rounded-sm border border-white/10 bg-[#0d1424]">
              <div className="flex flex-wrap items-center gap-3 border-b border-white/10 px-4 py-3">
                <span className="text-[#c6a15b]">
                  <GitBranch className="h-4 w-4" />
                </span>
                <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
                  Factory pipeline
                </h2>
                <span className="text-[11px] font-light text-slate-600">
                  {snapshot.pipeline.nodes.length} stories · {snapshot.pipeline.edges.length} dependency edges
                </span>

                <div className="ml-auto flex flex-wrap items-center gap-2">
                  <div className="relative">
                    <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-600" />
                    <input
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      placeholder="Story id or title…"
                      className="w-52 rounded-sm border border-white/10 bg-[#0a0f1a] py-1.5 pl-8 pr-3 text-xs text-slate-200 placeholder:text-slate-600 focus:border-[#c6a15b]/60 focus:outline-none"
                    />
                  </div>
                  <select
                    value={workstream}
                    onChange={(e) => setWorkstream(e.target.value as typeof workstream)}
                    className="rounded-sm border border-white/10 bg-[#0a0f1a] px-2.5 py-1.5 text-xs text-slate-300 focus:border-[#c6a15b]/60 focus:outline-none"
                  >
                    {WORKSTREAM_FILTERS.map((w) => (
                      <option key={w} value={w}>
                        {w === "All" ? "All workstreams" : w}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              <div className="max-h-[72vh] overflow-y-auto px-4 py-4">
                <div className="space-y-6">
                  {STAGE_ORDER.map((stage) => {
                    const meta = STAGE_META[stage]
                    const group = filteredNodes.filter((n) => n.stage === stage)
                    if (group.length === 0) return null
                    const isCollapsed = collapsed[stage]
                    return (
                      <div key={stage}>
                        <button
                          onClick={() =>
                            setCollapsed((c) => ({ ...c, [stage]: !c[stage] }))
                          }
                          className="flex w-full items-center gap-2 rounded-sm px-1 py-1 text-left"
                        >
                          <span className={`inline-block h-2 w-2 rounded-full ${meta.dot}`} />
                          <span className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
                            {meta.label}
                          </span>
                          <span className="tabular-nums text-[11px] font-light text-slate-600">
                            {group.length}
                          </span>
                          {isCollapsed ? (
                            <ChevronRight className="h-3.5 w-3.5 text-slate-600" />
                          ) : (
                            <ChevronDown className="h-3.5 w-3.5 text-slate-600" />
                          )}
                          <span className="hidden text-[10px] font-light text-slate-700 md:inline">
                            {meta.hint}
                          </span>
                        </button>

                        {!isCollapsed && (
                          <div className="mt-2 grid gap-2.5 lg:grid-cols-2 2xl:grid-cols-3">
                            {group.map((node) => (
                              <PipelineCard
                                key={node.storyId}
                                node={node}
                                edgesByFrom={edgesByFrom}
                                ready={readyStoryIds.has(node.storyId)}
                                gated={gatedStoryIds.has(node.storyId)}
                                onOpen={() =>
                                  router.push(`/portal/command-console/${encodeURIComponent(node.storyId)}`)
                                }
                              />
                            ))}
                          </div>
                        )}
                      </div>
                    )
                  })}
                  {filteredNodes.length === 0 && (
                    <div className="py-10 text-center text-xs font-light text-slate-600">
                      No stories match the current filter.
                    </div>
                  )}
                </div>
              </div>
            </section>
          </main>

          {/* Layer 2 — agent dispatch / capacity */}
          <aside className="space-y-5">
            <AgentCapacity
              snapshot={snapshot}
              activeWorker={activeWorker}
              blockedWorker={blockedWorker}
              idle={idle}
              gatedNodes={gatedNodes}
              onOpenStory={(id) =>
                router.push(`/portal/command-console/${encodeURIComponent(id)}`)
              }
            />
          </aside>
        </div>

        {/* Deep drill-down — workflow cockpits (engine instances) */}
        {snapshot.workflowCockpits.length > 0 && (
          <WorkflowCockpits summaries={snapshot.workflowCockpits} />
        )}

        <p className="pb-4 text-center text-[10px] font-light tracking-[0.18em] text-slate-700">
          ENG-16 factory command center — every number is a read projection over the canonical
          Story Board control plane. No duplicate state is introduced.
        </p>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Executive rollup (layer 1)
// ---------------------------------------------------------------------------

function ExecutiveRollup({
  snapshot,
  rollup,
}: {
  snapshot: FactorySnapshot
  rollup: NonNullable<FactorySnapshot["rollup"]>
}) {
  const hardBlocked = rollup.totalBlockedFailed
  const waitingOnDeps = snapshot.pipeline.blockedWork.filter((id) => {
    const node = snapshot.pipeline.nodes.find((n) => n.storyId === id)
    return node && node.status !== "Blocked" && node.status !== "Failed"
  }).length

  const kpis = [
    {
      label: "Complete",
      value: rollup.totalComplete,
      cls: "text-emerald-300",
      hint: "stories closed with evidence",
    },
    {
      label: "In progress",
      value: rollup.totalInProgressPartial,
      cls: "text-sky-300",
      hint: "flowing through the factory",
    },
    {
      label: "Ready",
      value: rollup.totalReady,
      cls: "text-[#e3c98a]",
      hint: `${snapshot.pipeline.readyWork.length} dependency-ready now`,
    },
    {
      label: "Blocked / waiting",
      value: hardBlocked + waitingOnDeps,
      cls: "text-red-300",
      hint: waitingOnDeps > 0 ? `${waitingOnDeps} waiting on dependencies` : "need human attention",
    },
  ]

  return (
    <section className="rounded-sm border border-white/10 bg-[#0d1424]">
      <div className="flex items-center gap-2 border-b border-white/10 px-4 py-3">
        <Gauge className="h-4 w-4 text-[#c6a15b]" />
        <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
          Executive rollup
        </h2>
        <span className="ml-auto text-[10px] font-light text-slate-600">
          net-net over {rollup.totalStories} stored stories
        </span>
      </div>

      <div className="grid gap-4 px-4 py-4 lg:grid-cols-[minmax(0,220px)_minmax(0,1fr)_auto]">
        <div className="flex items-center gap-3">
          <div>
            <div className="text-[10px] font-light uppercase tracking-[0.2em] text-slate-500">
              Net-net
            </div>
            <div className="mt-1 text-4xl font-light tabular-nums text-white">
              {rollup.netNet}
              <span className="text-lg text-slate-500">%</span>
            </div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {kpis.map((kpi) => (
            <div key={kpi.label} className="rounded-sm border border-white/10 bg-[#0a0f1a] px-3 py-2.5">
              <div className={`text-2xl font-light tabular-nums ${kpi.cls}`}>{kpi.value}</div>
              <div className="mt-0.5 text-[10px] font-light uppercase tracking-[0.16em] text-slate-500">
                {kpi.label}
              </div>
              <div className="mt-0.5 text-[10px] font-light text-slate-600">{kpi.hint}</div>
            </div>
          ))}
        </div>

        <div className="min-w-[220px] space-y-1.5">
          {rollup.workstreams.map((ws) => (
            <div key={ws.code} className="flex items-center gap-2" title={`${ws.completionPercent}% · ${ws.completeCount} complete / ${ws.openCount} open / ${ws.blockedCount} blocked`}>
              <span className="w-14 shrink-0 text-right font-mono text-[10px] text-slate-500">
                {ws.code}
              </span>
              <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-white/5">
                <div
                  className="h-full rounded-full bg-[#c6a15b]/80"
                  style={{ width: `${Math.max(0, Math.min(100, ws.completionPercent))}%` }}
                />
              </div>
              <span className="w-8 shrink-0 tabular-nums text-[10px] text-slate-400">
                {ws.completionPercent}%
              </span>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

// ---------------------------------------------------------------------------
// Pipeline story card — worker + story + evidence together (layer 3)
// ---------------------------------------------------------------------------

function PipelineCard({
  node,
  edgesByFrom,
  ready,
  gated,
  onOpen,
}: {
  node: import("@/lib/factory-command-center-data").FactoryPipelineNode
  edgesByFrom: Map<string, Map<string, boolean | null>>
  ready: boolean
  gated: boolean
  onOpen: () => void
}) {
  const pill = statePill(node.status)
  const runPill = node.runResult ? runResultPill(node.runResult) : null
  const runPillCls = runPill ? statePill(runPill).cls : ""
  const refSatisfied = edgesByFrom.get(node.storyId)

  return (
    <article
      id={`pipeline-card-${node.storyId}`}
      onClick={onOpen}
      className="group cursor-pointer rounded-sm border border-white/10 bg-[#0a0f1a] p-3.5 transition-colors hover:border-[#c6a15b]/50 hover:bg-[#0d1424]"
    >
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-xs font-medium tracking-wide text-[#e3c98a]">
          {node.storyId}
        </span>
        <div className="flex items-center gap-1.5">
          {gated && (
            <span
              className="rounded-full border border-amber-400/40 bg-amber-400/10 px-2 py-0.5 text-[9px] font-light uppercase tracking-wide text-amber-300"
              title={`${node.gate} — requires human presence before/while executing`}
            >
              {node.gate}
            </span>
          )}
          <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${pill.cls}`}>
            {pill.label}
          </span>
        </div>
      </div>

      <h3 className="mt-1 line-clamp-2 text-sm font-light leading-5 text-slate-200 group-hover:text-white">
        {node.title}
      </h3>

      <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] font-light text-slate-500">
        <span className="uppercase tracking-wide">{node.priority}</span>
        <span className="tabular-nums">{node.completion}%</span>
        <span className="font-mono text-[10px]">{node.workstream}</span>
      </div>

      {/* Worker + work state — story and worker visible together */}
      <div className="mt-2.5 flex items-center gap-2 rounded-sm border border-white/5 bg-white/[0.02] px-2.5 py-1.5">
        {node.worker ? (
          <>
            <UserCheck className="h-3.5 w-3.5 text-emerald-400/80" />
            <span className="truncate text-xs font-light text-slate-200" title={node.worker}>
              {node.worker}
            </span>
            {node.role && (
              <span className="rounded-full border border-white/10 px-1.5 py-0.5 text-[9px] font-light text-slate-500">
                {node.role}
              </span>
            )}
          </>
        ) : (
          <>
            <User className="h-3.5 w-3.5 text-slate-600" />
            <span className="text-xs font-light text-slate-500">unassigned</span>
          </>
        )}
        {node.workState ? (
          <span
            className={`ml-auto rounded-full border px-1.5 py-0.5 text-[9px] font-light ${statePill(node.workState).cls}`}
          >
            {statePill(node.workState).label}
          </span>
        ) : (
          <span className="ml-auto text-[9px] font-light uppercase tracking-wide text-slate-700">
            not queued
          </span>
        )}
      </div>

      {/* Work product / evidence state (concise) */}
      <div className="mt-2 space-y-1 text-[10px] font-light leading-4 text-slate-500">
        {runPill ? (
          <div className="flex items-center gap-2">
            <span className={`rounded-full border px-1.5 py-0.5 text-[9px] font-light ${runPillCls}`}>
              {runPill}
            </span>
            {node.runCompletion != null && (
              <span className="tabular-nums">{node.runCompletion}%</span>
            )}
            {node.testsSummary && (
              <span className="truncate text-slate-500" title={node.testsSummary}>
                {node.testsSummary}
              </span>
            )}
            {node.commitHash && (
              <span className="font-mono text-[9px] text-slate-600" title={node.commitHash}>
                @{shortId(node.commitHash)}
              </span>
            )}
          </div>
        ) : (
          <div className="text-slate-700">no run evidence yet</div>
        )}
        {node.latestStep && (
          <div className="truncate text-slate-500" title={node.latestStep}>
            {node.latestStep}
          </div>
        )}
      </div>

      {/* Dependency edges — why blocked/unblocked */}
      {node.dependencyRefs.length > 0 && (
        <div className="mt-2.5 flex flex-wrap items-center gap-1">
          <span className="text-[9px] font-light uppercase tracking-[0.14em] text-slate-600">
            depends
          </span>
          {node.dependencyRefs.map((ref) => {
            const satisfied = refSatisfied?.get(ref) ?? null
            const cls =
              satisfied === true
                ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-300"
                : satisfied === false
                  ? "border-red-400/40 bg-red-400/10 text-red-300"
                  : "border-white/10 bg-white/5 text-slate-500"
            const label =
              satisfied === true
                ? "complete"
                : satisfied === false
                  ? "waiting"
                  : "external"
            return (
              <span
                key={ref}
                title={
                  satisfied === false
                    ? `${ref} is not complete — this edge is why the story waits`
                    : satisfied === true
                      ? `${ref} is Complete — this edge is satisfied`
                      : `${ref} is not tracked on the board — unverifiable, never blocks`
                }
                className={`rounded-full border px-1.5 py-0.5 font-mono text-[9px] ${cls}`}
              >
                {ref} · {label}
              </span>
            )
          })}
        </div>
      )}

      {/* Block reason */}
      {node.blockedReason && (
        <div className="mt-2 flex items-start gap-1.5 rounded-sm border border-red-400/25 bg-red-400/5 px-2 py-1.5">
          <AlertTriangle className="mt-0.5 h-3 w-3 shrink-0 text-red-400/80" />
          <span className="text-[10px] font-light leading-4 text-red-200/90">{node.blockedReason}</span>
        </div>
      )}

      <div className="mt-2 flex items-center justify-between text-[9px] font-light text-slate-600">
        <span className="inline-flex items-center gap-1 text-[#c6a15b]/80 opacity-0 transition-opacity group-hover:opacity-100">
          Open cockpit <ExternalLink className="h-3 w-3" />
        </span>
        {node.blockedBy.length > 0 && (
          <span className="tabular-nums text-slate-700">
            {node.blockedBy.length} unmet dep{node.blockedBy.length > 1 ? "s" : ""}
          </span>
        )}
      </div>
    </article>
  )
}

// ---------------------------------------------------------------------------
// Agent dispatch / capacity (layer 2)
// ---------------------------------------------------------------------------

function AgentCapacity({
  snapshot,
  activeWorker,
  blockedWorker,
  idle,
  gatedNodes,
  onOpenStory,
}: {
  snapshot: FactorySnapshot
  activeWorker: import("@/lib/factory-command-center-data").WorkerCapacityEntry | null
  blockedWorker: import("@/lib/factory-command-center-data").WorkerCapacityEntry | null
  idle: import("@/lib/factory-command-center-data").WorkerCapacityEntry | null
  gatedNodes: import("@/lib/factory-command-center-data").FactoryPipelineNode[]
  onOpenStory: (id: string) => void
}) {
  return (
    <>
      <section className="rounded-sm border border-white/10 bg-[#0d1424]">
        <div className="flex items-center gap-2 border-b border-white/10 px-4 py-3">
          <UserCheck className="h-4 w-4 text-[#c6a15b]" />
          <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
            Agent dispatch / capacity
          </h2>
        </div>

        <div className="space-y-2.5 px-4 py-4">
          {activeWorker && (
            <div className="rounded-sm border border-emerald-400/25 bg-emerald-400/5 p-3">
              <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.16em] text-emerald-300">
                <UserCheck className="h-3.5 w-3.5" /> Assigned · busy
              </div>
              <div className="mt-1.5 text-sm font-light text-slate-100">
                {activeWorker.workerId ?? "unnamed worker"}
              </div>
              <button
                onClick={() => activeWorker.storyId && onOpenStory(activeWorker.storyId)}
                className="mt-1 block truncate font-mono text-[11px] text-[#e3c98a] hover:underline"
              >
                {activeWorker.storyId}
              </button>
              <div className="mt-2 flex items-center gap-2 text-[10px] font-light text-slate-500">
                <span className={`rounded-full border px-1.5 py-0.5 ${activeWorker.workState ? statePill(activeWorker.workState).cls : ""}`}>
                  {activeWorker.workState ?? "active"}
                </span>
                {activeWorker.role && <span>{activeWorker.role}</span>}
                <span className="ml-auto tabular-nums">{formatTime(activeWorker.since)}</span>
              </div>
            </div>
          )}

          {blockedWorker && (
            <div className="rounded-sm border border-red-400/30 bg-red-400/5 p-3">
              <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.16em] text-red-300">
                <UserX className="h-3.5 w-3.5" /> Blocked · needs human
              </div>
              <div className="mt-1.5 text-sm font-light text-slate-100">
                {blockedWorker.workerId ?? "unnamed worker"}
              </div>
              <button
                onClick={() => blockedWorker.storyId && onOpenStory(blockedWorker.storyId)}
                className="mt-1 block truncate font-mono text-[11px] text-[#e3c98a] hover:underline"
              >
                {blockedWorker.storyId}
              </button>
              <div className="mt-2 text-[10px] font-light text-slate-500">
                {blockedWorker.workState === "Error" ? "command failed — resolve then re-queue" : "run cancelled by operator"}
              </div>
            </div>
          )}

          {idle && (
            <div className="rounded-sm border border-white/10 bg-[#0a0f1a] p-3">
              <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.16em] text-slate-400">
                <User className="h-3.5 w-3.5" /> Available · idle
              </div>
              <div className="mt-1.5 text-sm font-light text-slate-300">
                No command in flight
              </div>
              <div className="mt-1 text-[10px] font-light leading-4 text-slate-600">
                {snapshot.capacity.nextEligible.length > 0
                  ? `${snapshot.capacity.nextEligible.length} command${snapshot.capacity.nextEligible.length > 1 ? "s" : ""} eligible to claim now.`
                  : "No dependency-ready work queued right now."}
              </div>
            </div>
          )}

          {!activeWorker && !blockedWorker && !idle && (
            <div className="rounded-sm border border-white/10 bg-[#0a0f1a] p-3 text-[10px] font-light text-slate-600">
              No worker state to show.
            </div>
          )}
        </div>
      </section>

      {/* Next eligible */}
      <section className="rounded-sm border border-white/10 bg-[#0d1424]">
        <div className="flex items-center gap-2 border-b border-white/10 px-4 py-3">
          <Activity className="h-4 w-4 text-[#c6a15b]" />
          <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
            Eligible next
          </h2>
        </div>
        <div className="space-y-1 px-3 py-3">
          {snapshot.capacity.nextEligible.length === 0 ? (
            <p className="px-1 py-2 text-[11px] font-light text-slate-600">
              Nothing dependency-ready is queued. Set a story to Ready on the Story Board.
            </p>
          ) : (
            snapshot.capacity.nextEligible.map((s) => (
              <button
                key={s.storyId}
                onClick={() => onOpenStory(s.storyId)}
                className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left transition-colors hover:bg-white/5"
              >
                <span className="font-mono text-[10px] text-[#e3c98a]">{s.storyId}</span>
                <span className="min-w-0 flex-1 truncate text-[11px] font-light text-slate-300" title={s.title}>
                  {s.title}
                </span>
                <span className="shrink-0 text-[9px] font-light uppercase tracking-wide text-slate-600">
                  {s.priority}
                </span>
              </button>
            ))
          )}
        </div>
      </section>

      {/* Human gates */}
      {gatedNodes.length > 0 && (
        <section className="rounded-sm border border-amber-400/20 bg-[#0d1424]">
          <div className="flex items-center gap-2 border-b border-white/10 px-4 py-3">
            <ShieldAlert className="h-4 w-4 text-amber-400" />
            <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
              Human gates
            </h2>
            <span className="ml-auto tabular-nums text-[10px] font-light text-amber-300/80">
              {gatedNodes.length}
            </span>
          </div>
          <div className="space-y-1 px-3 py-3">
            {gatedNodes.map((n) => (
              <button
                key={n.storyId}
                onClick={() => onOpenStory(n.storyId)}
                className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left transition-colors hover:bg-white/5"
              >
                <span className="font-mono text-[10px] text-[#e3c98a]">{n.storyId}</span>
                <span className="min-w-0 flex-1 truncate text-[11px] font-light text-slate-300" title={n.title}>
                  {n.title}
                </span>
                <span className="shrink-0 rounded-full border border-amber-400/30 px-1.5 py-0.5 text-[9px] font-light text-amber-300">
                  {n.gate}
                </span>
              </button>
            ))}
          </div>
        </section>
      )}
    </>
  )
}

// ---------------------------------------------------------------------------
// Deep drill-down — workflow cockpits (engine workflow instances, ENG-15)
// ---------------------------------------------------------------------------

function workflowOutcomePill(summary: WorkflowSummary): { label: string; cls: string } {
  if (summary.outcome === "cancelled") return { label: "Cancelled", cls: "bg-slate-500/15 text-slate-400 border-slate-500/30" }
  if (summary.outcome === "failed" || summary.outcome === "conflict" || summary.status === "error") {
    return { label: summary.outcome ?? summary.status, cls: "bg-red-400/15 text-red-300 border-red-400/40" }
  }
  if (summary.outcome === "completed") return { label: "Closed", cls: "bg-emerald-400/15 text-emerald-300 border-emerald-400/30" }
  return { label: summary.status, cls: "bg-sky-400/15 text-sky-300 border-sky-400/30" }
}

function WorkflowCockpits({ summaries }: { summaries: WorkflowSummary[] }) {
  return (
    <section className="rounded-sm border border-white/10 bg-[#0d1424]">
      <div className="flex items-center gap-2 border-b border-white/10 px-4 py-3">
        <GitBranch className="h-4 w-4 text-[#c6a15b]" />
        <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">
          Deep workflow cockpits
        </h2>
        <span className="text-[10px] font-light text-slate-600">
          live engine instances — drill into the workflow cockpit for each
        </span>
        <span className="ml-auto tabular-nums text-[10px] font-light text-slate-500">
          {summaries.length}
        </span>
      </div>
      <div className="grid gap-2.5 px-4 py-4 lg:grid-cols-2 2xl:grid-cols-3">
        {summaries.slice(0, 9).map((s) => {
          const pill = workflowOutcomePill(s)
          return (
            <Link
              key={s.instanceId}
              href={`/portal/workflows/${s.instanceId}`}
              className="group rounded-sm border border-white/10 bg-[#0a0f1a] p-3 transition-colors hover:border-[#c6a15b]/50"
            >
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 text-[9px] font-light uppercase tracking-[0.16em] text-slate-500">
                    <GitBranch className="h-3 w-3" />
                    {s.workflowName} · v{s.workflowVersion}
                  </div>
                  <div className="mt-1 truncate text-sm font-light text-slate-200">
                    {s.propertyName ?? "Transaction"}
                  </div>
                </div>
                <span className={`shrink-0 rounded-full border px-2 py-0.5 text-[9px] font-light capitalize ${pill.cls}`}>
                  {pill.label}
                </span>
              </div>
              <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] font-light text-slate-500">
                {s.activeMilestones.length > 0 ? (
                  <span className="flex items-center gap-1">
                    <Clock className="h-3 w-3" />
                    {s.activeMilestones.join(", ")}
                  </span>
                ) : (
                  <span>{s.responsibleParty ?? "no active milestone"}</span>
                )}
                {s.blockerCount > 0 && (
                  <span className="flex items-center gap-1 text-amber-300">
                    <AlertTriangle className="h-3 w-3" /> {s.blockerCount} blocker{s.blockerCount > 1 ? "s" : ""}
                  </span>
                )}
                {s.openTaskCount > 0 && (
                  <span className="flex items-center gap-1">
                    <CheckCircle2 className="h-3 w-3" /> {s.openTaskCount} open
                  </span>
                )}
                <span className="ml-auto inline-flex items-center gap-1 text-[#c6a15b]/80 opacity-0 transition-opacity group-hover:opacity-100">
                  Open cockpit <ExternalLink className="h-3 w-3" />
                </span>
              </div>
            </Link>
          )
        })}
      </div>
    </section>
  )
}

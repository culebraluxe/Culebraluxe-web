'use client';

import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  buildCausalGraph,
  layoutGraph,
  type CausalLayout,
  type GraphColor,
} from '@/lib/causal-graph';
import {
  KIND_TOKENS,
  SYSTEM_CHIP,
  toTimelineEntries,
  type ConsoleWorkflowNode,
  type ConsoleWorkflowView,
  type EventKind,
  type FlightRecorderTrace,
  type MainTab,
  type SystemId,
  type TraceEvent,
} from '@/lib/flight-recorder-adapter';
import { formatClock, formatDisplayTime, formatDuration, formatOffset } from './format';
import { KindGlyph } from './KindGlyph';
import { useFlightRecorderState } from './useFlightRecorderState';

const TABS: { id: MainTab; label: string }[] = [
  { id: 'timeline', label: 'Timeline' },
  { id: 'causality', label: 'Causality Graph' },
  { id: 'swimlane', label: 'System Swimlane' },
  { id: 'raw', label: 'Raw Events' },
];

const KIND_ORDER: EventKind[] = [
  'Command',
  'DomainEvent',
  'Workflow',
  'Task',
  'Integration',
  'Persistence',
];

/** Minimal structural slice of the virtualizer the timeline actually uses,
 *  so the component isn't coupled to the library's generic parameters. */
type VirtualizerLike = {
  getTotalSize: () => number;
  getVirtualItems: () => Array<{ index: number; start: number; key: unknown }>;
};

export function FlightRecorderPage({
  trace,
  defaultEventId,
}: {
  trace: FlightRecorderTrace;
  defaultEventId?: string;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const {
    selectedEventId,
    tab,
    density,
    filters,
    setSelectedEventId,
    setTab,
    setDensity,
    setQuery,
    toggleKind,
    clearFilters,
    filteredEvents,
    selectedEvent,
  } = useFlightRecorderState(trace.events, defaultEventId);

  // Cross-selection between the Trace Map (master workflow nodes) and the
  // Timeline (real events): selecting a node emphasizes its events; selecting an
  // event highlights its workflow node (by immutable workflowNodeId).
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const selectEvent = useCallback(
    (id: string) => {
      setSelectedEventId(id);
      const ev = trace.events.find((e) => e.id === id);
      setSelectedNodeId(ev?.workflowNodeId ?? null);
    },
    [trace.events, setSelectedEventId],
  );
  const selectNode = useCallback((nodeId: string) => {
    setSelectedNodeId((cur) => (cur === nodeId ? null : nodeId));
  }, []);

  const rowHeight = density === 'compact' ? 56 : 88;

  const virtualizer = useVirtualizer({
    count: filteredEvents.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 12,
    getItemKey: (index) => filteredEvents[index]?.id ?? index,
  });

  useEffect(() => {
    virtualizer.measure();
  }, [density, virtualizer]);

  useEffect(() => {
    if (!selectedEventId) return;
    const index = filteredEvents.findIndex((e) => e.id === selectedEventId);
    if (index >= 0) virtualizer.scrollToIndex(index, { align: 'center' });
  }, [selectedEventId, filteredEvents, virtualizer]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const idx = filteredEvents.findIndex((ev) => ev.id === selectedEventId);
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        const next = filteredEvents[Math.min(idx + 1, filteredEvents.length - 1)];
        if (next) selectEvent(next.id);
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        const prev = filteredEvents[Math.max(idx - 1, 0)];
        if (prev) selectEvent(prev.id);
      }
      if (e.key === 'Escape') {
        setSelectedEventId(null);
        setSelectedNodeId(null);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [filteredEvents, selectedEventId, selectEvent, setSelectedEventId, setSelectedNodeId]);

  const kindCounts = useMemo(() => {
    const counts = Object.fromEntries(KIND_ORDER.map((k) => [k, 0])) as Record<EventKind, number>;
    for (const e of trace.events) counts[e.kind] += 1;
    return counts;
  }, [trace.events]);

  return (
    <div className="flex h-screen flex-col bg-[#0b1220] text-slate-100">
      <Header query={filters.query} onQuery={setQuery} />

      <div className="flex min-h-0 flex-1">
        <LeftRail
          trace={trace}
          kindCounts={kindCounts}
          activeKinds={filters.kinds}
          onToggleKind={toggleKind}
          selectedEventId={selectedEventId}
          selectedNodeId={selectedNodeId}
          onSelect={selectEvent}
          onSelectNode={selectNode}
        />

        <main className="flex min-w-0 flex-1 flex-col border-x border-white/5">
          <div className="flex items-center justify-between gap-3 border-b border-white/5 px-4">
            <nav className="flex gap-1" aria-label="Trace views">
              {TABS.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => setTab(t.id)}
                  className={`relative px-3 py-3 text-sm ${
                    tab === t.id ? 'text-white' : 'text-slate-400 hover:text-slate-200'
                  }`}
                >
                  {t.label}
                  {tab === t.id && (
                    <span className="absolute inset-x-2 -bottom-px h-0.5 rounded bg-sky-400" />
                  )}
                </button>
              ))}
            </nav>

            <div className="flex items-center gap-2 py-2">
              <button
                type="button"
                onClick={clearFilters}
                className="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300"
              >
                Filter{filters.kinds.length || filters.query ? ` (${filteredEvents.length})` : ''}
              </button>
              <div className="flex rounded-md border border-white/10 p-0.5 text-xs">
                {(['compact', 'expanded'] as const).map((d) => (
                  <button
                    key={d}
                    type="button"
                    onClick={() => setDensity(d)}
                    className={`rounded px-2.5 py-1 capitalize ${
                      density === d ? 'bg-slate-700 text-white' : 'text-slate-400'
                    }`}
                  >
                    {d}
                  </button>
                ))}
              </div>
              <button
                type="button"
                onClick={() => downloadJson(trace, `flight-recorder-${trace.summary.correlationId}.json`)}
                className="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300"
              >
                Export
              </button>
              <button
                type="button"
                onClick={() => downloadJson(filteredEvents, `flight-recorder-filtered-${trace.summary.correlationId}.json`)}
                className="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300"
              >
                Download
              </button>
            </div>
          </div>

          {tab === 'timeline' ? (
            <TimelineTable
              parentRef={parentRef}
              events={filteredEvents}
              virtualizer={virtualizer}
              selectedEventId={selectedEventId}
              highlightNodeId={selectedNodeId}
              onSelect={selectEvent}
              density={density}
              rowHeight={rowHeight}
            />
          ) : tab === 'causality' ? (
            <CausalityGraph
              events={filteredEvents}
              selectedEventId={selectedEventId}
              onSelect={selectEvent}
            />
          ) : tab === 'swimlane' ? (
            <Swimlane
              events={filteredEvents}
              selectedEventId={selectedEventId}
              onSelect={selectEvent}
            />
          ) : (
            <RawEvents events={filteredEvents} />
          )}
        </main>

        <EventDetails event={selectedEvent} onSelect={selectEvent} onTag={(tag) => setQuery(tag)} />
      </div>
    </div>
  );
}

function Header({ query, onQuery }: { query: string; onQuery: (q: string) => void }) {
  return (
    <header className="flex h-14 shrink-0 items-center gap-4 border-b border-white/5 px-4">
      <div className="flex items-center gap-2">
        <span className="grid h-8 w-8 place-items-center rounded-lg bg-slate-800 text-sm">FR</span>
        <div>
          <div className="text-sm font-semibold">Flight Recorder</div>
          <div className="text-[11px] text-slate-400">End-to-end transaction timeline & causality</div>
        </div>
      </div>
      <div className="mx-auto w-full max-w-xl">
        <input
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          placeholder="Search events, entities, correlations..."
          className="w-full rounded-lg border border-white/10 bg-slate-900/80 px-3 py-1.5 text-sm outline-none ring-sky-500/40 placeholder:text-slate-500 focus:ring-2"
        />
      </div>
      <div className="flex items-center gap-2 text-xs">
        <span className="rounded-full border border-white/10 px-2.5 py-1 text-emerald-300">Auto Refresh · On</span>
        <span className="rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1 text-emerald-300">
          Live
        </span>
        <button type="button" className="rounded-full border border-white/10 px-2.5 py-1">
          Share
        </button>
      </div>
    </header>
  );
}

function LeftRail({
  trace,
  kindCounts,
  activeKinds,
  onToggleKind,
  selectedEventId,
  selectedNodeId,
  onSelect,
  onSelectNode,
}: {
  trace: FlightRecorderTrace;
  kindCounts: Record<EventKind, number>;
  activeKinds: EventKind[];
  onToggleKind: (k: EventKind) => void;
  selectedEventId: string | null;
  selectedNodeId: string | null;
  onSelect: (id: string) => void;
  onSelectNode: (id: string) => void;
}) {
  const { summary } = trace;
  return (
    <aside className="flex w-[280px] shrink-0 flex-col gap-4 overflow-y-auto p-4">
      <div>
        <div className="text-[10px] uppercase tracking-wide text-slate-500">Correlation ID</div>
        <div className="mt-1 break-all font-mono text-[11px] text-slate-200">{summary.correlationId}</div>
        <div className="mt-2 flex items-center gap-2 text-xs">
          <span className="text-slate-400">Root: {summary.rootTitle}</span>
          <span className={`rounded-full px-2 py-0.5 text-[10px] ${KIND_TOKENS[summary.rootKind].chip}`}>
            {KIND_TOKENS[summary.rootKind].label}
          </span>
        </div>
      </div>

      <dl className="grid grid-cols-4 gap-2 text-center">
        {[
          ['Duration', formatDuration(summary.durationMs)],
          ['Events', String(summary.eventCount)],
          ['Systems', String(summary.systemCount)],
          ['Status', summary.status],
        ].map(([k, v]) => (
          <div key={k}>
            <dt className="text-[10px] uppercase text-slate-500">{k}</dt>
            <dd className={`mt-0.5 text-sm ${k === 'Status' ? 'text-emerald-400' : 'text-white'}`}>{v}</dd>
          </div>
        ))}
      </dl>

      <section>
        <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Business Context</h2>
        <dl className="mt-2 space-y-1.5 text-xs">
          {Object.entries({
            Deal: summary.businessContext.deal ?? summary.businessContext.dealId,
            Property: summary.businessContext.property,
            Client: summary.businessContext.client,
            Workflow: summary.businessContext.workflow,
            'Initiated By': summary.businessContext.initiatedBy,
            At: summary.businessContext.initiatedAt
              ? formatDisplayTime(summary.businessContext.initiatedAt)
              : undefined,
          })
            .filter(([, v]) => v != null)
            .map(([k, v]) => (
              <div key={k} className="grid grid-cols-[88px_1fr] gap-2">
                <dt className="text-slate-500">{k}</dt>
                <dd className="text-slate-200">{v}</dd>
              </div>
            ))}
        </dl>
      </section>

      <section>
        <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Master Workflow</h2>
        <TraceMapMini
          workflow={trace.workflow}
          selectedNodeId={selectedNodeId}
          onSelectNode={onSelectNode}
        />
      </section>

      <section>
        <ul className="space-y-1 text-xs">
          {KIND_ORDER.map((kind) => (
            <li key={kind}>
              <button
                type="button"
                onClick={() => onToggleKind(kind)}
                className={`flex w-full items-center justify-between rounded px-1 py-0.5 ${
                  activeKinds.includes(kind) ? 'bg-white/5' : ''
                }`}
              >
                <span className="flex items-center gap-2">
                  <span className={`h-2.5 w-2.5 rounded-full ${KIND_TOKENS[kind].bg}`} />
                  {KIND_TOKENS[kind].label}
                </span>
                <span className="text-slate-500">{kindCounts[kind]}</span>
              </button>
            </li>
          ))}
        </ul>
      </section>
    </aside>
  );
}

function TraceMapMini({
  workflow,
  selectedNodeId,
  onSelectNode,
}: {
  workflow?: ConsoleWorkflowView;
  selectedNodeId: string | null;
  onSelectNode: (id: string) => void;
}) {
  const layout = useMemo(() => (workflow ? layoutMasterWorkflow(workflow) : null), [workflow]);

  if (!workflow || !layout || workflow.nodes.length === 0) {
    return (
      <div className="mt-2 rounded-lg border border-white/5 bg-slate-900/40 p-3 text-xs text-slate-500">
        No master workflow to map.
      </div>
    );
  }

  const stateFill: Record<ConsoleWorkflowNode['state'], string> = {
    COMPLETED: '#34d399',
    CURRENT: '#c6a15b',
    FAILED: '#f87171',
    RECOVERED: '#fbbf24',
    NOT_VISITED: '#475569',
  };

  return (
    <div className="mt-2 h-52 overflow-auto rounded-lg border border-white/5 bg-slate-900/40">
      <svg
        width={layout.width}
        height={layout.height}
        viewBox={`0 0 ${layout.width} ${layout.height}`}
        className="min-w-full"
      >
        {layout.edges.map((e, i) => (
          <line
            key={i}
            x1={e.x1}
            y1={e.y1}
            x2={e.x2}
            y2={e.y2}
            stroke="rgba(255,255,255,0.15)"
            strokeWidth={1}
          />
        ))}
        {layout.nodes.map((n) => {
          const selected = n.id === selectedNodeId;
          return (
            <g key={n.id} transform={`translate(${n.x},${n.y})`}>
              <circle
                r={layout.nodeRadius + (selected ? 2 : 0)}
                fill={stateFill[n.state]}
                opacity={n.state === 'NOT_VISITED' ? 0.4 : 0.9}
                stroke={
                  selected ? '#fff' : n.state === 'CURRENT' ? 'rgba(255,255,255,0.9)' : 'rgba(255,255,255,0.25)'
                }
                strokeWidth={selected ? 2 : n.state === 'CURRENT' ? 1.5 : 1}
              />
              <circle
                r={layout.nodeRadius + 7}
                fill="transparent"
                style={{ cursor: 'pointer' }}
                onClick={() => onSelectNode(n.id)}
              >
                <title>{`${n.name} — ${n.state}`}</title>
              </circle>
              <text y={-layout.nodeRadius - 4} textAnchor="middle" fontSize={8} className="fill-slate-300">
                {n.name}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

/** Deterministic layered layout of the exact persisted master workflow. */
function layoutMasterWorkflow(workflow: ConsoleWorkflowView): {
  width: number;
  height: number;
  nodeRadius: number;
  nodes: { id: string; name: string; state: ConsoleWorkflowNode['state']; x: number; y: number }[];
  edges: { x1: number; y1: number; x2: number; y2: number }[];
} {
  const nodeRadius = 10;
  const layerW = 170;
  const rowH = 58;
  const pad = 30;
  const incoming = new Map<string, number>();
  for (const t of workflow.transitions) incoming.set(t.to, (incoming.get(t.to) ?? 0) + 1);
  const starts = workflow.nodes.filter((n) => (incoming.get(n.id) ?? 0) === 0);
  const byId = new Map(workflow.nodes.map((n) => [n.id, n]));
  const layers: string[][] = [];
  const seen = new Set<string>();
  let frontier = (starts.length ? starts : workflow.nodes.slice(0, 1)).map((n) => n.id);
  while (frontier.length > 0) {
    const layer = frontier.filter((id) => !seen.has(id));
    if (layer.length === 0) break;
    layers.push(layer);
    layer.forEach((id) => seen.add(id));
    const next: string[] = [];
    for (const id of layer) {
      for (const t of workflow.transitions) {
        if (t.from === id && !seen.has(t.to) && byId.has(t.to)) next.push(t.to);
      }
    }
    frontier = next;
  }
  for (const n of workflow.nodes) if (!seen.has(n.id)) layers.push([n.id]);
  const pos = new Map<string, { x: number; y: number }>();
  layers.forEach((layer, li) => {
    layer.forEach((id, ri) => pos.set(id, { x: pad + li * layerW, y: pad + ri * rowH }));
  });
  const width = pad * 2 + layers.length * layerW;
  const maxRows = Math.max(1, ...layers.map((l) => l.length));
  const height = pad * 2 + maxRows * rowH;
  const nodes = workflow.nodes.map((n) => {
    const p = pos.get(n.id) ?? { x: pad, y: pad };
    return { id: n.id, name: n.name, state: n.state, x: p.x, y: p.y };
  });
  const edges = workflow.transitions
    .filter((t) => pos.has(t.from) && pos.has(t.to))
    .map((t) => {
      const a = pos.get(t.from)!;
      const b = pos.get(t.to)!;
      return { x1: a.x, y1: a.y, x2: b.x, y2: b.y };
    });
  return { width, height, nodeRadius, nodes, edges };
}

// Shared causal-DAG renderer. TraceMapMini and the full-pane Causality tab both
// reuse the engine's pure layout and stay selection-synchronized with the console.
function CausalGraphSvg({
  layout,
  selectedEventId,
  onSelect,
}: {
  layout: CausalLayout;
  selectedEventId: string | null;
  onSelect: (id: string) => void;
}) {
  const FILL: Record<GraphColor, string> = {
    command: '#a78bfa',
    domain: '#60a5fa',
    workflow: '#34d399',
    task: '#c6a15b',
    external: '#f472b6',
    persistence: '#22d3ee',
    failure: '#f87171',
    neutral: '#94a3b8',
  };

  return (
    <svg
      width={layout.width}
      height={layout.height}
      viewBox={`0 0 ${layout.width} ${layout.height}`}
      className="min-w-full"
    >
      {layout.edges.map((edge) => {
        const a = layout.nodes.find((n) => n.id === edge.source);
        const b = layout.nodes.find((n) => n.id === edge.target);
        if (!a || !b) return null;
        return (
          <line
            key={`${edge.source}->${edge.target}`}
            x1={a.x}
            y1={a.y}
            x2={b.x}
            y2={b.y}
            stroke="rgba(255,255,255,0.18)"
            strokeWidth={1}
          />
        );
      })}
      {layout.nodes.map((n) => {
        const selected = n.members.some((m) => m.id === selectedEventId);
        return (
          <g key={n.id} transform={`translate(${n.x},${n.y})`}>
            <circle
              r={layout.nodeRadius + (selected ? 2 : 0)}
              fill={FILL[n.color]}
              opacity={selected ? 1 : 0.85}
              stroke={selected ? '#fff' : 'rgba(255,255,255,0.25)'}
              strokeWidth={selected ? 2 : 1}
            />
            <circle
              r={layout.nodeRadius + 7}
              fill="transparent"
              style={{ cursor: 'pointer' }}
              onClick={() => onSelect(n.members[0].id)}
            >
              <title>{n.label}</title>
            </circle>
            <text
              y={-layout.nodeRadius - 4}
              textAnchor="middle"
              fontSize={8}
              className="fill-slate-300"
            >
              {n.count > 1 ? `${n.label} ×${n.count}` : n.label}
            </text>
          </g>
        );
      })}
    </svg>
  );
}


// Full-pane causality graph for the Causality tab. Uses the same filtered
// selection as the timeline so the graph and the list stay in sync.
function CausalityGraph({
  events,
  selectedEventId,
  onSelect,
}: {
  events: TraceEvent[];
  selectedEventId: string | null;
  onSelect: (id: string) => void;
}) {
  const timeline = useMemo(() => toTimelineEntries(events), [events]);
  const graph = useMemo(() => buildCausalGraph(timeline), [timeline]);
  const layout = useMemo(() => layoutGraph(graph), [graph]);

  if (layout.nodes.length === 0) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-slate-500">
        No causal links in the current selection.
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <CausalGraphSvg layout={layout} selectedEventId={selectedEventId} onSelect={onSelect} />
    </div>
  );
}

// System swimlane: one lane per SystemId, events placed by elapsed offset so the
// parallel flow across producers is readable at a glance.
function Swimlane({
  events,
  selectedEventId,
  onSelect,
}: {
  events: TraceEvent[];
  selectedEventId: string | null;
  onSelect: (id: string) => void;
}) {
  const { systems, maxMs } = useMemo(() => {
    const order: SystemId[] = [];
    const seen = new Set<string>();
    for (const e of events) {
      if (!seen.has(e.system)) {
        seen.add(e.system);
        order.push(e.system);
      }
    }
    return { systems: order, maxMs: Math.max(1, ...events.map((e) => e.offsetMs)) };
  }, [events]);

  if (events.length === 0) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-slate-500">
        No events in the current selection.
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1 space-y-4 overflow-auto p-4">
      <div className="flex items-center gap-2 text-[10px] uppercase tracking-wide text-slate-500">
        <span>Offset</span>
        <span className="text-slate-600">0 → {maxMs} ms</span>
      </div>
      {systems.map((system) => (
        <div key={system}>
          <div className="mb-1 flex items-center gap-2">
            <span className={`inline-flex rounded-md px-2 py-0.5 text-[10px] ${SYSTEM_CHIP[system]}`}>
              {system}
            </span>
          </div>
          <div className="relative h-12 rounded-lg border border-white/5 bg-slate-900/40">
            {events
              .filter((e) => e.system === system)
              .map((e) => {
                const left = (e.offsetMs / maxMs) * 100;
                const selected = e.id === selectedEventId;
                return (
                  <button
                    key={e.id}
                    type="button"
                    title={`${e.title} (${e.kind}, ${e.status})`}
                    aria-label={e.title}
                    onClick={() => onSelect(e.id)}
                    className={`absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full ${KIND_TOKENS[e.kind].bg} ${
                      selected ? 'ring-2 ring-white' : ''
                    }`}
                    style={{ left: `${left}%` }}
                  />
                );
              })}
          </div>
        </div>
      ))}
    </div>
  );
}

// Raw Events: the adapted console read-model as a deterministic JSON dump.
function RawEvents({ events }: { events: TraceEvent[] }) {
  return (
    <div className="min-h-0 flex-1 overflow-auto p-4">
      <pre className="rounded-lg bg-[#0a1018] p-4 font-mono text-[11px] leading-relaxed text-slate-300">
        {JSON.stringify(events, null, 2)}
      </pre>
    </div>
  );
}

// Export/Download helper: serialize a JSON payload as a client-side file.
function downloadJson(data: unknown, filename: string) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}


function TimelineTable({
  parentRef,
  events,
  virtualizer,
  selectedEventId,
  highlightNodeId,
  onSelect,
  density,
  rowHeight,
}: {
  parentRef: RefObject<HTMLDivElement | null>;
  events: TraceEvent[];
  virtualizer: VirtualizerLike;
  selectedEventId: string | null;
  highlightNodeId: string | null;
  onSelect: (id: string) => void;
  density: 'compact' | 'expanded';
  rowHeight: number;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="grid grid-cols-[108px_minmax(0,1.2fr)_minmax(0,1.4fr)_140px] border-b border-white/5 px-3 py-2 text-[10px] uppercase tracking-wide text-slate-500">
        <div>Time</div>
        <div>Event</div>
        <div>Details</div>
        <div>System</div>
      </div>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-auto" role="grid" aria-rowcount={events.length}>
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((row) => {
            const event = events[row.index];
            if (!event) return null;
            const selected = event.id === selectedEventId;
            const matchesNode = highlightNodeId != null && event.workflowNodeId === highlightNodeId;
            const detailEntries = Object.entries(event.details);
            return (
              <button
                key={event.id}
                type="button"
                role="row"
                aria-selected={selected}
                onClick={() => onSelect(event.id)}
                className={`absolute left-0 grid w-full grid-cols-[108px_minmax(0,1.2fr)_minmax(0,1.4fr)_140px] items-center overflow-hidden border-b border-white/5 px-3 text-left text-sm ${
                  selected
                    ? 'bg-slate-800'
                    : matchesNode
                      ? 'bg-amber-500/10'
                      : 'hover:bg-slate-900/80'
                } ${matchesNode ? 'border-l-2 border-l-amber-400' : ''}`}
                style={{ transform: `translateY(${row.start}px)`, height: rowHeight }}
              >
                <div className="font-mono text-[11px] leading-tight text-slate-300">
                  <div>{formatClock(event.occurredAt)}</div>
                  <div className="text-slate-500">{formatOffset(event.offsetMs)}</div>
                </div>
                <div className="flex min-w-0 items-center gap-2">
                  <span className={`grid h-7 w-7 shrink-0 place-items-center rounded-md text-white ${KIND_TOKENS[event.kind].bg}`}>
                    <KindGlyph kind={event.kind} />
                  </span>
                  <div className="min-w-0">
                    <div className="truncate font-medium">{event.title}</div>
                    <div className="truncate text-[11px] text-slate-500">{event.subtitle}</div>
                  </div>
                </div>
                <div className="min-w-0 text-[12px] leading-snug text-slate-300">
                  {(density === 'compact' ? detailEntries.slice(0, 2) : detailEntries).map(([k, v]) => (
                    <div key={k} className="truncate">
                      <span className="text-slate-500">{k}: </span>
                      {v}
                    </div>
                  ))}
                </div>
                <div>
                  <span className={`inline-flex rounded-md px-2 py-0.5 text-[11px] ${SYSTEM_CHIP[event.system]}`}>
                    {event.system}
                  </span>
                </div>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function EventDetails({
  event,
  onSelect,
  onTag,
}: {
  event: TraceEvent | null;
  onSelect: (id: string) => void;
  onTag: (tag: string) => void;
}) {
  if (!event) {
    return (
      <aside className="w-[360px] shrink-0 p-6 text-sm text-slate-500">Select an event in the timeline</aside>
    );
  }

  const overview: [string, string][] = [
    ['Event ID', event.id],
    ['Time', formatDisplayTime(event.occurredAt)],
    ['Duration', formatDuration(event.durationMs)],
    ['System', event.system],
    ['Correlation ID', event.correlationId],
    ['Causation ID', event.causationId ?? '—'],
    ['Event Type', event.type],
  ];

  return (
    <aside className="flex w-[360px] shrink-0 flex-col overflow-y-auto border-l border-white/5">
      <div className="flex items-start justify-between gap-3 border-b border-white/5 p-4">
        <div className="flex items-start gap-2">
          <span className={`grid h-8 w-8 place-items-center rounded-md text-white ${KIND_TOKENS[event.kind].bg}`}>
            <KindGlyph kind={event.kind} />
          </span>
          <div>
            <div className="font-medium">{event.title}</div>
            <div className="text-xs text-slate-400">{KIND_TOKENS[event.kind].label} Event</div>
          </div>
        </div>
        <span className="rounded-full bg-emerald-500/15 px-2 py-0.5 text-[11px] text-emerald-300">{event.status}</span>
      </div>

      <section className="border-b border-white/5 p-4">
        <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Overview</h2>
        <dl className="mt-2 space-y-1.5 text-xs">
          {overview.map(([k, v]) => (
            <div key={k} className="grid grid-cols-[108px_1fr] gap-2">
              <dt className="text-slate-500">{k}</dt>
              <dd className="break-all font-mono text-[11px] text-slate-200">
                {k === 'Causation ID' && event.causationId ? (
                  <button type="button" className="text-sky-300 hover:underline" onClick={() => onSelect(event.causationId!)}>
                    {v}
                  </button>
                ) : (
                  v
                )}
              </dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="border-b border-white/5 p-4">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Payload</h2>
          <button
            type="button"
            className="text-[11px] text-slate-400 hover:text-white"
            onClick={() => navigator.clipboard.writeText(JSON.stringify(event.payload, null, 2))}
          >
            Copy
          </button>
        </div>
        <pre className="overflow-x-auto rounded-lg bg-[#0a1018] p-3 font-mono text-[11px] leading-relaxed text-slate-300">
          {JSON.stringify(event.payload, null, 2)}
        </pre>
      </section>

      <section className="border-b border-white/5 p-4">
        <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Tags</h2>
        <div className="mt-2 flex flex-wrap gap-1.5">
          {event.tags.map((tag) => (
            <button
              key={tag}
              type="button"
              onClick={() => onTag(tag)}
              className="rounded-full border border-white/10 px-2 py-0.5 text-[11px] text-slate-300 hover:bg-white/5"
            >
              {tag}
            </button>
          ))}
        </div>
      </section>

      <section className="p-4">
        <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Related Events</h2>
        <ul className="mt-2 space-y-1">
          {event.relatedEventIds.map((rel) => (
            <li key={rel.id}>
              <button
                type="button"
                onClick={() => onSelect(rel.id)}
                className="flex w-full items-center justify-between rounded px-1 py-1 text-left text-xs text-sky-300 hover:bg-white/5"
              >
                <span>{rel.title}</span>
                <span className="font-mono text-[11px] text-slate-500">{formatOffset(rel.offsetMs)}</span>
              </button>
            </li>
          ))}
        </ul>
      </section>
    </aside>
  );
}

export default FlightRecorderPage;

'use client';

import { useEffect, useMemo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DEAL_TRACE_FIXTURE, SELECTED_EVENT_ID } from './fixture';
import { formatClock, formatDisplayTime, formatDuration, formatOffset } from './format';
import { KindGlyph } from './KindGlyph';
import { KIND_TOKENS, SYSTEM_CHIP, type EventKind, type MainTab, type TraceEvent } from './types';
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

export function FlightRecorderPage({
  trace = DEAL_TRACE_FIXTURE,
}: {
  trace?: typeof DEAL_TRACE_FIXTURE;
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
  } = useFlightRecorderState(trace.events, SELECTED_EVENT_ID);

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
        if (next) setSelectedEventId(next.id);
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        const prev = filteredEvents[Math.max(idx - 1, 0)];
        if (prev) setSelectedEventId(prev.id);
      }
      if (e.key === 'Escape') setSelectedEventId(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [filteredEvents, selectedEventId, setSelectedEventId]);

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
          onSelect={setSelectedEventId}
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
              <button type="button" className="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300">
                Export
              </button>
              <button type="button" className="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300">
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
              onSelect={setSelectedEventId}
              density={density}
              rowHeight={rowHeight}
            />
          ) : (
            <div className="grid flex-1 place-items-center text-sm text-slate-500">
              {TABS.find((t) => t.id === tab)?.label} view — wire to the same `filteredEvents` graph.
            </div>
          )}
        </main>

        <EventDetails event={selectedEvent} onSelect={setSelectedEventId} onTag={(tag) => setQuery(tag)} />
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
  onSelect,
}: {
  trace: typeof DEAL_TRACE_FIXTURE;
  kindCounts: Record<EventKind, number>;
  activeKinds: EventKind[];
  onToggleKind: (k: EventKind) => void;
  selectedEventId: string | null;
  onSelect: (id: string) => void;
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
            Deal: summary.businessContext.dealId,
            Property: summary.businessContext.property,
            Client: summary.businessContext.client,
            Workflow: summary.businessContext.workflow,
            'Initiated By': summary.businessContext.initiatedBy,
            At: formatDisplayTime(summary.businessContext.initiatedAt),
          }).map(([k, v]) => (
            <div key={k} className="grid grid-cols-[88px_1fr] gap-2">
              <dt className="text-slate-500">{k}</dt>
              <dd className="text-slate-200">{v}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section>
        <h2 className="text-[10px] uppercase tracking-wide text-slate-500">Trace Map</h2>
        <TraceMapMini events={trace.events} selectedEventId={selectedEventId} onSelect={onSelect} />
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
  events,
  selectedEventId,
  onSelect,
}: {
  events: TraceEvent[];
  selectedEventId: string | null;
  onSelect: (id: string) => void;
}) {
  const byParent = new Map<string | null, TraceEvent[]>();
  for (const e of events) {
    const key = e.causationId;
    const list = byParent.get(key) ?? [];
    list.push(e);
    byParent.set(key, list);
  }

  const levels: TraceEvent[][] = [];
  let frontier = byParent.get(null) ?? [];
  const seen = new Set<string>();
  while (frontier.length) {
    levels.push(frontier);
    frontier.forEach((e) => seen.add(e.id));
    frontier = frontier.flatMap((e) => byParent.get(e.id) ?? []).filter((e) => !seen.has(e.id));
  }

  return (
    <div className="mt-2 rounded-lg border border-white/5 bg-slate-900/40 p-3">
      <div className="flex flex-col items-center gap-3">
        {levels.map((level, i) => (
          <div key={i} className="flex flex-wrap justify-center gap-2">
            {level.map((e) => (
              <button
                key={e.id}
                type="button"
                title={e.title}
                onClick={() => onSelect(e.id)}
                className={`grid h-7 w-7 place-items-center rounded-md text-white ${KIND_TOKENS[e.kind].bg} ${
                  selectedEventId === e.id ? 'ring-2 ring-white' : 'opacity-80 hover:opacity-100'
                }`}
              >
                <KindGlyph kind={e.kind} className="h-3.5 w-3.5" />
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

function TimelineTable({
  parentRef,
  events,
  virtualizer,
  selectedEventId,
  onSelect,
  density,
  rowHeight,
}: {
  parentRef: React.RefObject<HTMLDivElement | null>;
  events: TraceEvent[];
  virtualizer: ReturnType<typeof useVirtualizer>;
  selectedEventId: string | null;
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
            const detailEntries = Object.entries(event.details);
            return (
              <button
                key={event.id}
                type="button"
                role="row"
                aria-selected={selected}
                onClick={() => onSelect(event.id)}
                className={`absolute left-0 grid w-full grid-cols-[108px_minmax(0,1.2fr)_minmax(0,1.4fr)_140px] items-center border-b border-white/5 px-3 text-left text-sm ${
                  selected ? 'bg-slate-800' : 'hover:bg-slate-900/80'
                }`}
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

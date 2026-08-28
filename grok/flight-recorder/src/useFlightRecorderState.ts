'use client';

import { useCallback, useMemo } from 'react';
import { usePathname, useRouter, useSearchParams } from 'next/navigation';
import type { EventKind, MainTab, SystemId, TimelineDensity, TraceEvent } from './types';

export interface FlightRecorderFilters {
  kinds: EventKind[];
  systems: SystemId[];
  tags: string[];
  query: string;
}

export interface FlightRecorderState {
  selectedEventId: string | null;
  tab: MainTab;
  density: TimelineDensity;
  filters: FlightRecorderFilters;
  setSelectedEventId: (id: string | null) => void;
  setTab: (tab: MainTab) => void;
  setDensity: (density: TimelineDensity) => void;
  setQuery: (query: string) => void;
  toggleKind: (kind: EventKind) => void;
  clearFilters: () => void;
}

const TABS: MainTab[] = ['timeline', 'causality', 'swimlane', 'raw'];

function parseTab(raw: string | null): MainTab {
  return TABS.includes(raw as MainTab) ? (raw as MainTab) : 'timeline';
}

function parseList<T extends string>(raw: string | null, allow: readonly T[]): T[] {
  if (!raw) return [];
  return raw.split(',').filter((v): v is T => allow.includes(v as T));
}

export function useFlightRecorderState(
  events: TraceEvent[],
  defaultEventId?: string,
): FlightRecorderState & { filteredEvents: TraceEvent[]; selectedEvent: TraceEvent | null } {
  const router = useRouter();
  const pathname = usePathname();
  const params = useSearchParams();

  const selectedEventId = params.get('event') ?? defaultEventId ?? null;
  const tab = parseTab(params.get('tab'));
  const density: TimelineDensity = params.get('density') === 'expanded' ? 'expanded' : 'compact';
  const query = params.get('q') ?? '';
  const kinds = parseList(params.get('kinds'), [
    'Command',
    'DomainEvent',
    'Workflow',
    'Task',
    'Integration',
    'Persistence',
  ] as const);
  const systems = parseList(params.get('systems'), [
    'API Gateway',
    'Domain Model',
    'Workflow Engine',
    'Task Service',
    'BoldSign',
    'PostgreSQL',
  ] as const);
  const tags = params.get('tags')?.split(',').filter(Boolean) ?? [];

  const replace = useCallback(
    (patch: Record<string, string | null>) => {
      const next = new URLSearchParams(params.toString());
      for (const [k, v] of Object.entries(patch)) {
        if (v === null || v === '') next.delete(k);
        else next.set(k, v);
      }
      const qs = next.toString();
      router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false });
    },
    [params, pathname, router],
  );

  const setSelectedEventId = useCallback(
    (id: string | null) => replace({ event: id }),
    [replace],
  );
  const setTab = useCallback((t: MainTab) => replace({ tab: t === 'timeline' ? null : t }), [replace]);
  const setDensity = useCallback(
    (d: TimelineDensity) => replace({ density: d === 'compact' ? null : d }),
    [replace],
  );
  const setQuery = useCallback((q: string) => replace({ q }), [replace]);
  const toggleKind = useCallback(
    (kind: EventKind) => {
      const next = kinds.includes(kind) ? kinds.filter((k) => k !== kind) : [...kinds, kind];
      replace({ kinds: next.length ? next.join(',') : null });
    },
    [kinds, replace],
  );
  const clearFilters = useCallback(
    () => replace({ kinds: null, systems: null, tags: null, q: null }),
    [replace],
  );

  const filteredEvents = useMemo(() => {
    const q = query.trim().toLowerCase();
    return events.filter((e) => {
      if (kinds.length && !kinds.includes(e.kind)) return false;
      if (systems.length && !systems.includes(e.system)) return false;
      if (tags.length && !tags.every((t) => e.tags.includes(t))) return false;
      if (!q) return true;
      const hay = [
        e.title,
        e.type,
        e.system,
        e.id,
        ...Object.values(e.details),
        JSON.stringify(e.payload),
      ]
        .join(' ')
        .toLowerCase();
      return hay.includes(q);
    });
  }, [events, kinds, systems, tags, query]);

  const selectedEvent = events.find((e) => e.id === selectedEventId) ?? null;

  return {
    selectedEventId,
    tab,
    density,
    filters: { kinds, systems, tags, query },
    setSelectedEventId,
    setTab,
    setDensity,
    setQuery,
    toggleKind,
    clearFilters,
    filteredEvents,
    selectedEvent,
  };
}

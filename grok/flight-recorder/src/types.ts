export type EventKind =
  | 'Command'
  | 'DomainEvent'
  | 'Workflow'
  | 'Task'
  | 'Integration'
  | 'Persistence';

export type SystemId =
  | 'API Gateway'
  | 'Domain Model'
  | 'Workflow Engine'
  | 'Task Service'
  | 'BoldSign'
  | 'PostgreSQL';

export type EventStatus = 'Success' | 'Failed' | 'Pending' | 'Skipped';

export interface RelatedEventRef {
  id: string;
  title: string;
  /** Signed offset from the selected event, in milliseconds. */
  offsetMs: number;
}

export interface TraceEvent {
  id: string;
  correlationId: string;
  causationId: string | null;
  parentIds?: string[];
  kind: EventKind;
  type: string;
  title: string;
  subtitle: string;
  system: SystemId;
  status: EventStatus;
  occurredAt: string;
  offsetMs: number;
  durationMs: number;
  details: Record<string, string>;
  payload: unknown;
  tags: string[];
  relatedEventIds: RelatedEventRef[];
}

export interface BusinessContext {
  dealId: string;
  property: string;
  client: string;
  workflow: string;
  initiatedBy: string;
  initiatedAt: string;
}

export interface TraceSummary {
  correlationId: string;
  rootTitle: string;
  rootKind: EventKind;
  durationMs: number;
  eventCount: number;
  systemCount: number;
  status: 'Completed' | 'Failed' | 'InProgress';
  businessContext: BusinessContext;
}

export interface FlightRecorderTrace {
  summary: TraceSummary;
  events: TraceEvent[];
}

export type TimelineDensity = 'compact' | 'expanded';
export type MainTab = 'timeline' | 'causality' | 'swimlane' | 'raw';

export const KIND_TOKENS: Record<
  EventKind,
  { bg: string; text: string; chip: string; label: string }
> = {
  Command: {
    bg: 'bg-violet-500',
    text: 'text-violet-300',
    chip: 'bg-violet-500/20 text-violet-200 ring-1 ring-violet-400/30',
    label: 'Command',
  },
  DomainEvent: {
    bg: 'bg-sky-500',
    text: 'text-sky-300',
    chip: 'bg-sky-500/20 text-sky-200 ring-1 ring-sky-400/30',
    label: 'Domain Event',
  },
  Workflow: {
    bg: 'bg-emerald-500',
    text: 'text-emerald-300',
    chip: 'bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30',
    label: 'Workflow',
  },
  Task: {
    bg: 'bg-amber-500',
    text: 'text-amber-300',
    chip: 'bg-amber-500/20 text-amber-200 ring-1 ring-amber-400/30',
    label: 'Task',
  },
  Integration: {
    bg: 'bg-fuchsia-500',
    text: 'text-fuchsia-300',
    chip: 'bg-fuchsia-500/20 text-fuchsia-200 ring-1 ring-fuchsia-400/30',
    label: 'Integration',
  },
  Persistence: {
    bg: 'bg-teal-500',
    text: 'text-teal-300',
    chip: 'bg-teal-500/20 text-teal-200 ring-1 ring-teal-400/30',
    label: 'Persistence',
  },
};

export const SYSTEM_CHIP: Record<SystemId, string> = {
  'API Gateway': 'bg-indigo-500/20 text-indigo-200 ring-1 ring-indigo-400/30',
  'Domain Model': 'bg-blue-500/20 text-blue-200 ring-1 ring-blue-400/30',
  'Workflow Engine': 'bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30',
  'Task Service': 'bg-amber-600/20 text-amber-200 ring-1 ring-amber-400/30',
  BoldSign: 'bg-pink-500/20 text-pink-200 ring-1 ring-pink-400/30',
  PostgreSQL: 'bg-cyan-500/20 text-cyan-200 ring-1 ring-cyan-400/30',
};

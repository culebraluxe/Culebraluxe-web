import type { EventKind } from '@/lib/flight-recorder-adapter';

const PATHS: Record<EventKind, string> = {
  Command: 'M12 2l2.4 7.2H22l-6 4.4 2.3 7.4L12 16.8 5.7 21l2.3-7.4L2 9.2h7.6z',
  DomainEvent: 'M12 3a9 9 0 100 18 9 9 0 000-18zm0 4v5l4 2',
  Workflow: 'M7 7h4v4H7zM13 7h4v4h-4zM7 13h4v4H7zM13 13h4v4h-4z',
  Task: 'M9 3h6l1 3h3v15H5V6h3l1-3zm0 8h6m-6 4h4',
  Integration: 'M7 12a5 5 0 015-5h2m3 5a5 5 0 01-5 5h-2M8 12h8',
  Persistence: 'M4 7a8 3 0 0016 0A8 3 0 004 7zm0 5c0 1.7 3.6 3 8 3s8-1.3 8-3M4 17c0 1.7 3.6 3 8 3s8-1.3 8-3',
};

export function KindGlyph({ kind, className = 'h-4 w-4' }: { kind: EventKind; className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" className={className}>
      <path d={PATHS[kind]} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

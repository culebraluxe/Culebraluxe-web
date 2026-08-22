// ---------------------------------------------------------------------------
// Shared dark/navy cockpit UI helpers (ENG-20) — extracted from the SDLC
// Command Console so the Story Execution Cockpit reuses the exact same pill /
// time / id formatting. No visual-polish scope here; functional only.
// ---------------------------------------------------------------------------

export function statePill(state: string): { label: string; cls: string } {
  const map: Record<string, { label: string; cls: string }> = {
    Ready: { label: 'Ready', cls: 'bg-sky-400/15 text-sky-300 border-sky-400/30' },
    Claimed: { label: 'Claimed', cls: 'bg-amber-400/15 text-amber-300 border-amber-400/30' },
    Running: { label: 'Running', cls: 'bg-emerald-400/15 text-emerald-300 border-emerald-400/30' },
    Paused: { label: 'Paused', cls: 'bg-slate-400/15 text-slate-300 border-slate-400/30' },
    Done: { label: 'Done', cls: 'bg-emerald-400/15 text-emerald-300 border-emerald-400/30' },
    Error: { label: 'Error', cls: 'bg-red-400/15 text-red-300 border-red-400/40' },
    Cancelled: { label: 'Cancelled', cls: 'bg-slate-500/15 text-slate-400 border-slate-500/30' },
    Planned: { label: 'Planned', cls: 'bg-white/5 text-white/45 border-white/10' },
    Complete: { label: 'Complete', cls: 'bg-emerald-400/15 text-emerald-300 border-emerald-400/30' },
    Partial: { label: 'Partial', cls: 'bg-amber-400/15 text-amber-300 border-amber-400/30' },
    Blocked: { label: 'Blocked', cls: 'bg-red-400/15 text-red-300 border-red-400/40' },
    Failed: { label: 'Failed', cls: 'bg-red-400/15 text-red-300 border-red-400/40' },
    'In Progress': { label: 'In Progress', cls: 'bg-emerald-400/15 text-emerald-300 border-emerald-400/30' },
  }
  return map[state] ?? { label: state, cls: 'bg-white/5 text-white/50 border-white/10' }
}

export function shortId(id: string | null | undefined): string {
  if (!id) return '—'
  if (id.length <= 24) return id
  return `${id.slice(0, 10)}…${id.slice(-6)}`
}

export function formatTime(iso: string | null | undefined): string {
  if (!iso) return '—'
  const d = new Date(iso)
  // Defensive: legacy/malformed timestamps must never crash the cockpit
  // (RangeError: Invalid time value).
  if (Number.isNaN(d.getTime())) return '—'
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function runResultPill(status: string | null): string {
  if (status === 'Complete') return 'Complete'
  if (status === 'Failed' || status === 'Cancelled') return status
  return status ?? 'Pending'
}

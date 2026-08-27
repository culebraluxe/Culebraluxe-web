// ---------------------------------------------------------------------------
// ACCOUNTING V1 — display helpers (pure, testable). No data access.
// ---------------------------------------------------------------------------

export function formatMoney(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value)
}

/** 'YYYY-MM-DD' (or ISO) → 'Aug 27, 2026'. */
export function formatDate(value: string | null): string {
  if (!value) return ''
  const date = new Date(value.length === 10 ? `${value}T00:00:00` : value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  })
}

export function todayISO(): string {
  return new Date().toISOString().slice(0, 10)
}

export function startOfMonthISO(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-01`
}

export function endOfMonthISO(): string {
  const d = new Date(new Date().getFullYear(), new Date().getMonth() + 1, 0)
  return d.toISOString().slice(0, 10)
}

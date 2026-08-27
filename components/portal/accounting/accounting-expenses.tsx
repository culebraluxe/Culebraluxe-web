'use client'

import { useActionState, useMemo, useState } from 'react'

import type { Expense } from '@/db/accounting'
import { EXPENSE_CATEGORIES } from '@/lib/accounting/categories'
import { formatDate, formatMoney, todayISO } from '@/lib/accounting/format'
import { createExpenseAction, type AccountingWriteState } from '@/app/portal/accounting/actions'
import { GlassPanel } from '@/components/portal/accounting/accounting-shell'

const emptyCreate: AccountingWriteState = null

const DONUT_COLORS = [
  '#c6a15b',
  '#7dd3fc',
  '#f0abfc',
  '#86efac',
  '#fcd34d',
  '#fca5a5',
  '#a5b4fc',
  '#93c5fd',
  '#e2e8f0',
]

export function AccountingExpenses({ rows }: { rows: Expense[] }) {
  const [showNew, setShowNew] = useState(false)
  const [createState, createAction, createPending] = useActionState(
    createExpenseAction,
    emptyCreate,
  )

  const byCategory = useMemo(() => {
    const map = new Map<string, number>()
    for (const e of rows) {
      if (e.status === 'POSTED') map.set(e.category, (map.get(e.category) ?? 0) + e.amount)
    }
    return [...map.entries()].sort((a, b) => b[1] - a[1])
  }, [rows])
  const total = byCategory.reduce((s, [, v]) => s + v, 0)

  // conic-gradient donut built from the category breakdown
  let cursor = 0
  const gradientStops = byCategory
    .map(([, value], i) => {
      const frac = total ? value / total : 0
      const from = cursor
      const to = cursor + frac * 360
      cursor = to
      return `${DONUT_COLORS[i % DONUT_COLORS.length]} ${from}deg ${to}deg`
    })
    .join(', ')

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm font-light text-white/60">
          {rows.length} expense{rows.length === 1 ? '' : 's'}
        </p>
        <button
          type="button"
          onClick={() => setShowNew((v) => !v)}
          className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/60 bg-[var(--portal-gold)]/15 px-4 py-2 text-xs font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:bg-[var(--portal-gold)]/25"
        >
          {showNew ? 'Cancel' : '+ New Expense'}
        </button>
      </div>

      {showNew && (
        <GlassPanel title="New Expense">
          <form
            action={createAction}
            className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3"
          >
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Vendor *
              <input
                name="vendor"
                required
                placeholder="e.g. Broker Public Portal"
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Category *
              <select
                name="category"
                required
                defaultValue=""
                className="mt-1 w-full rounded-md border border-white/15 bg-[#0b1220] px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
              >
                <option value="" disabled>
                  Select category…
                </option>
                {EXPENSE_CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Amount (USD) *
              <input
                name="amount"
                type="number"
                min="0"
                step="0.01"
                required
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Date
              <input
                name="expenseOn"
                type="date"
                defaultValue={todayISO()}
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60 sm:col-span-2">
              Memo
              <input
                name="memo"
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <div className="flex items-end gap-3 sm:col-span-2 lg:col-span-3">
              <button
                type="submit"
                disabled={createPending}
                className="rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110 disabled:opacity-50"
              >
                {createPending ? 'Creating…' : 'Create Expense'}
              </button>
              {createState?.error && (
                <p className="text-xs text-rose-300">{createState.error}</p>
              )}
              {createState?.ok && (
                <p className="text-xs text-emerald-300">Created.</p>
              )}
            </div>
          </form>
        </GlassPanel>
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <GlassPanel title="Expenses by Category" className="lg:col-span-1">
          {total > 0 ? (
            <div className="flex items-center gap-5">
              <div
                className="relative h-32 w-32 flex-none rounded-full"
                style={{
                  background: `conic-gradient(${gradientStops})`,
                }}
              >
                <div className="absolute inset-[22%] flex items-center justify-center rounded-full bg-[var(--portal-navy-deep)]">
                  <span className="text-[10px] font-medium uppercase tracking-wide text-white/50">
                    {byCategory.length} cats
                  </span>
                </div>
              </div>
              <ul className="min-w-0 flex-1 space-y-1.5">
                {byCategory.map(([label, value], i) => (
                  <li key={label} className="flex items-center gap-2 text-xs">
                    <span
                      className="h-2 w-2 flex-none rounded-full"
                      style={{ background: DONUT_COLORS[i % DONUT_COLORS.length] }}
                    />
                    <span className="truncate text-white/70">{label}</span>
                    <span className="ml-auto flex-none text-white/90">
                      {formatMoney(value)}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          ) : (
            <p className="py-6 text-sm font-light text-white/40">
              No posted expenses to visualize.
            </p>
          )}
        </GlassPanel>

        <GlassPanel title="All Expenses" className="lg:col-span-2">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[640px] text-left text-sm">
              <thead>
                <tr className="border-b border-white/10 text-[10px] font-medium uppercase tracking-[0.16em] text-white/45">
                  <th className="py-2 pr-3">Date</th>
                  <th className="py-2 pr-3">Vendor</th>
                  <th className="py-2 pr-3">Category</th>
                  <th className="py-2 pr-3">Client / Property</th>
                  <th className="py-2 pr-3 text-right">Amount</th>
                  <th className="py-2 pr-3">Status</th>
                  <th className="py-2">Memo</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((e) => (
                  <tr
                    key={e.id}
                    className="border-b border-white/[0.06] hover:bg-white/[0.03]"
                  >
                    <td className="py-2.5 pr-3 text-white/60">
                      {formatDate(e.expenseOn)}
                    </td>
                    <td className="py-2.5 pr-3 text-white/85">{e.vendor}</td>
                    <td className="py-2.5 pr-3 text-white/70">{e.category}</td>
                    <td className="py-2.5 pr-3 text-white/70">
                      {e.dealName || e.propertyName || e.personName || '—'}
                    </td>
                    <td className="py-2.5 pr-3 text-right font-medium text-sky-300">
                      {formatMoney(e.amount)}
                    </td>
                    <td className="py-2.5 pr-3">
                      <span
                        className={`inline-flex rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide ${
                          e.status === 'POSTED'
                            ? 'border-emerald-400/40 text-emerald-300'
                            : e.status === 'VOID'
                              ? 'border-white/20 text-white/40'
                              : 'border-white/25 text-white/60'
                        }`}
                      >
                        {e.status}
                      </span>
                    </td>
                    <td className="py-2.5 text-white/50">
                      <span className="block max-w-[12rem] truncate">
                        {e.memo || '—'}
                      </span>
                    </td>
                  </tr>
                ))}
                {rows.length === 0 && (
                  <tr>
                    <td
                      colSpan={7}
                      className="py-6 text-center text-sm font-light text-white/40"
                    >
                      No expenses yet.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </GlassPanel>
      </div>
    </div>
  )
}

